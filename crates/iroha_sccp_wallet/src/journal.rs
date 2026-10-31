//! Resumable SCCP wallet journal keyed by Taira `NetworkId` (spec §7).
//!
//! Every flow journals its raw evidence and state transitions before it submits anything, so a
//! crashed flow resumes where it stopped and never blindly resubmits. The journal reuses
//! `iroha_operation_journal::Journal` for every step: one owner-private directory whose
//! records are written once through retained native atomic publication and owner-only custody,
//! under an exclusive lock.
//!
//! Layout below the configured `journal_dir` (Unix directories `0700` and files `0600`;
//! Windows protected current-owner ACLs; no symlinks or reparse points):
//!
//! ```text
//! <journal_dir>/<network-id-hex>/<kind>-<flow-id-hex>/step-000000/operation.json
//!                                                                /submission.json
//!                                                                /applied.json
//!                                                    /step-000001/…
//! ```
//!
//! - The network directory is the lowercase hex of the Taira `NetworkId`, so records of a reset
//!   Taira (a new `NetworkId`, §4.18) never mix with the old ones.
//! - A flow is identified by its kind and a 32-byte id (the message id of a transfer, or a
//!   caller-chosen digest for committee sync and control relays).
//! - Steps are dense and append-only. `operation.json` is the [`SccpJournalRecordV1`]: the exact
//!   bytes about to be submitted and the raw evidence they were built from. `submission.json`
//!   is the pre-dispatch marker ([`SccpJournalStep::begin_submission`] returns `true` to exactly
//!   one caller). `applied.json` is the [`SccpJournalOutcomeV1`].
//! - Every record repeats the network id, the flow and the step index, and reading checks them
//!   against the directory it came from. Submission and completion also verify the exact
//!   stored operation before and after the journal action, including failed actions.
//!
//! A step that is prepared or submitted but has no outcome is the resume point
//! ([`SccpFlowJournal::resume_point`]): a submitted step is reconciled against the chain, never
//! replaced. The typed driver defaults to observation-only recovery; an explicit chain
//! adapter may rebroadcast identical retained bytes under the original nonce/seqno protection.
//!
//! Appending a step creates its directory (and lock) before it installs `operation.json`, so a
//! crash or I/O error in between leaves an empty step ([`SccpStepStateV1::Empty`]). Nothing was
//! dispatched from it, so it is also a resume point: appending the same index again reclaims the
//! directory. It is reopened under its lock when the lock exists; otherwise the directory is
//! removed and created afresh, and `rmdir` refuses to delete anything it holds.
//!
//! [`crate::flow`] validates the typed transfer history and drives exact-byte recovery.

use core::fmt;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_data_model::{NetworkId, bridge::SccpNetworkV1};
use iroha_operation_journal::Journal;
use norito::json;

/// Prefix of step directory names.
pub const STEP_DIR_PREFIX: &str = "step-";
/// Most steps of one flow (six decimal digits).
pub const MAX_STEPS: u32 = 999_999;
/// Largest journal record read back (the shared operation journal's own bound).
pub const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;

const OPERATION_FILE: &str = "operation.json";
const SUBMISSION_FILE: &str = "submission.json";
const APPLIED_FILE: &str = "applied.json";
/// Lock file `iroha_operation_journal::Journal::create` makes right after the directory.
const LOCK_FILE: &str = "lock";

/// Kind of a resumable flow.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(tag = "kind", content = "detail")]
pub enum SccpFlowKindV1 {
    /// Taira → external transfer (§7.1).
    #[norito(rename = "outbound")]
    Outbound,
    /// External → Taira transfer (§7.2).
    #[norito(rename = "inbound")]
    Inbound,
    /// Void and refund of an outbound transfer (§7.3).
    #[norito(rename = "refund")]
    Refund,
    /// Destination native committee synchronization (§7.4).
    #[norito(rename = "committee_sync")]
    CommitteeSync,
    /// Destination control relay (§7.4).
    #[norito(rename = "control_apply")]
    ControlApply,
    /// Submit original verified forgery evidence to Taira.
    #[norito(rename = "evidence_submission")]
    EvidenceSubmission,
    /// Deterministic destination deployment with its original committee pin.
    #[norito(rename = "deployment")]
    Deployment,
    /// Fund one retained TON recovery operation without repeating the original transfer.
    #[norito(rename = "ton_recovery")]
    TonRecovery,
}

impl SccpFlowKindV1 {
    /// Every kind.
    pub const ALL: [Self; 8] = [
        Self::Outbound,
        Self::Inbound,
        Self::Refund,
        Self::CommitteeSync,
        Self::ControlApply,
        Self::EvidenceSubmission,
        Self::Deployment,
        Self::TonRecovery,
    ];

    /// Directory-name label of the kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Outbound => "outbound",
            Self::Inbound => "inbound",
            Self::Refund => "refund",
            Self::CommitteeSync => "committee_sync",
            Self::ControlApply => "control_apply",
            Self::EvidenceSubmission => "evidence_submission",
            Self::Deployment => "deployment",
            Self::TonRecovery => "ton_recovery",
        }
    }

    /// The kind of a directory-name label.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == label)
    }
}

/// Identity of one flow within a network.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
pub struct SccpFlowIdV1 {
    /// Flow kind.
    pub kind: SccpFlowKindV1,
    /// Message id of a transfer, or a caller-chosen digest.
    pub id: [u8; 32],
}

impl SccpFlowIdV1 {
    /// `<kind>-<id-hex>`.
    #[must_use]
    pub fn dir_name(&self) -> String {
        format!("{}-{}", self.kind.as_str(), hex::encode(self.id))
    }

    /// Parse [`Self::dir_name`].
    #[must_use]
    pub fn from_dir_name(name: &str) -> Option<Self> {
        let (label, id) = name.rsplit_once('-')?;
        let kind = SccpFlowKindV1::from_label(label)?;
        if id.len() != 64 || id.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        let mut bytes = [0_u8; 32];
        hex::decode_to_slice(id, &mut bytes).ok()?;
        Some(Self { kind, id: bytes })
    }
}

/// One piece of raw evidence (a Torii response frame, an RPC result, a receipt).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
pub struct SccpEvidenceItemV1 {
    /// What the bytes are, e.g. `torii.message_proof` or `evm.committee_state`.
    pub label: String,
    /// The exact bytes as received.
    #[norito(json = "iroha_sccp::api::base64_json")]
    pub bytes: Vec<u8>,
}

/// A prepared step: what is about to be submitted and the evidence it was built from.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
pub struct SccpJournalRecordV1 {
    /// Taira `NetworkId` bytes.
    pub network_id: [u8; 32],
    /// The flow.
    pub flow: SccpFlowIdV1,
    /// Zero-based step index.
    pub step: u32,
    /// Chain the step is submitted to.
    pub chain: SccpNetworkV1,
    /// Closed action of the typed transfer or destination-maintenance state machine.
    pub action: crate::flow::SccpFlowActionV1,
    /// The exact bytes to submit (a signed transaction), or empty for a read-only step.
    #[norito(json = "iroha_sccp::api::base64_json")]
    pub submission: Vec<u8>,
    /// Raw evidence the submission was built and verified from.
    pub evidence: Vec<SccpEvidenceItemV1>,
}

/// How a step ended.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(tag = "outcome", content = "detail")]
pub enum SccpStepOutcomeKindV1 {
    /// Included and successful.
    #[norito(rename = "confirmed")]
    Confirmed,
    /// Included and reverted, or rejected by Taira.
    #[norito(rename = "failed")]
    Failed,
    /// Made unnecessary by state observed later (another party finalized or rotated first).
    #[norito(rename = "superseded")]
    Superseded,
}

/// The final record of a step, written once.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
pub struct SccpJournalOutcomeV1 {
    /// How the step ended.
    pub outcome: SccpStepOutcomeKindV1,
    /// Transaction hash or id on the step's chain, or empty.
    #[norito(json = "iroha_sccp::api::base64_json")]
    pub transaction: Vec<u8>,
    /// Raw evidence of the outcome (receipt, status response).
    pub evidence: Vec<SccpEvidenceItemV1>,
}

/// Progress of one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SccpStepStateV1 {
    /// The directory exists but `operation.json` was never installed (the append stopped
    /// between the two). Nothing was dispatched; append the same index again to reclaim it.
    Empty,
    /// Prepared; nothing was dispatched.
    Prepared,
    /// The pre-dispatch marker exists; reconcile before doing anything else.
    Submitted,
    /// The outcome is recorded.
    Completed(SccpStepOutcomeKindV1),
}

/// A step as found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpStepStatusV1 {
    /// Zero-based step index.
    pub index: u32,
    /// The prepared record; `None` exactly when the state is [`SccpStepStateV1::Empty`].
    pub record: Option<SccpJournalRecordV1>,
    /// Its progress.
    pub state: SccpStepStateV1,
}

/// Journal errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalError {
    /// A file-system operation failed.
    Io {
        /// What was being done.
        context: &'static str,
        /// The I/O error kind.
        kind: std::io::ErrorKind,
    },
    /// A journal path is a symlink, not owner-only, owned by another user, or of the wrong type.
    Unsafe(PathBuf),
    /// A record does not belong where it was found, or steps are not dense.
    Inconsistent(String),
    /// A record is not canonical Norito JSON of its type.
    Malformed(PathBuf),
    /// The underlying shared operation journal refused an operation.
    Storage(String),
}

impl fmt::Display for JournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, kind } => write!(formatter, "SCCP journal: {context}: {kind}"),
            Self::Unsafe(path) => write!(
                formatter,
                "SCCP journal path `{}` must be an owner-only, non-symlink entry of the current user",
                path.display()
            ),
            Self::Inconsistent(message) | Self::Storage(message) => {
                write!(formatter, "SCCP journal: {message}")
            }
            Self::Malformed(path) => write!(
                formatter,
                "SCCP journal record `{}` is not a canonical record",
                path.display()
            ),
        }
    }
}

impl std::error::Error for JournalError {}

fn io(context: &'static str) -> impl FnOnce(std::io::Error) -> JournalError {
    move |error| JournalError::Io {
        context,
        kind: error.kind(),
    }
}

fn storage(error: impl fmt::Display) -> JournalError {
    JournalError::Storage(format!("{error:#}"))
}

/// Original native directory authority shared by cloned journal selectors.
#[derive(Debug, Clone)]
struct RetainedDirectory {
    owner: Arc<iroha_fs::PrivateDirectory>,
    identity: iroha_fs::FileIdentity,
}

impl RetainedDirectory {
    fn new(owner: iroha_fs::PrivateDirectory) -> Result<Self, JournalError> {
        let identity = owner
            .identity()
            .map_err(|error| custody(owner.path(), error))?;
        Ok(Self {
            owner: Arc::new(owner),
            identity,
        })
    }
}

impl std::ops::Deref for RetainedDirectory {
    type Target = iroha_fs::PrivateDirectory;
    fn deref(&self) -> &Self::Target {
        &self.owner
    }
}

impl PartialEq for RetainedDirectory {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity && self.path() == other.path()
    }
}
impl Eq for RetainedDirectory {}

fn custody(path: &Path, error: std::io::Error) -> JournalError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::PermissionDenied
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::InvalidData
            | std::io::ErrorKind::IsADirectory
            | std::io::ErrorKind::NotADirectory
    ) {
        JournalError::Unsafe(path.to_owned())
    } else {
        io("retain private journal custody")(error)
    }
}

/// Read one record through the original private step directory.
fn read_private_file(
    directory: &iroha_fs::PrivateDirectory,
    name: &str,
) -> Result<Option<Vec<u8>>, JournalError> {
    use std::io::Read as _;

    let path = directory.path().join(name);
    let Some(mut original) = directory
        .open_retained_private_optional(name)
        .map_err(|error| custody(&path, error))?
    else {
        return Ok(None);
    };
    let result = (|| {
        #[cfg(windows)]
        original
            .require_current_owner()
            .map_err(|error| custody(&path, error))?;
        let length = original
            .file()
            .metadata()
            .map_err(io("inspect original record"))?
            .len();
        if length > MAX_RECORD_BYTES {
            return Err(JournalError::Malformed(path.clone()));
        }
        let before = original.snapshot().map_err(|error| custody(&path, error))?;
        let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
        original
            .file_mut()
            .take(MAX_RECORD_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io("read original record"))?;
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(JournalError::Malformed(path.clone()));
        }
        if bytes.len() as u64 != length
            || original.snapshot().map_err(|error| custody(&path, error))? != before
        {
            return Err(JournalError::Unsafe(path.clone()));
        }
        Ok(Some(bytes))
    })();
    original
        .revalidate()
        .map_err(|error| custody(&path, error))?;
    directory
        .revalidate()
        .map_err(|error| custody(directory.path(), error))?;
    result
}

#[cfg(test)]
fn create_private_dir(path: &Path) -> Result<(), JournalError> {
    let parent = iroha_fs::PrivateDirectory::open(path.parent().expect("step parent"))
        .map_err(|error| custody(path, error))?;
    parent
        .create_child(path.file_name().expect("step name"))
        .map(drop)
        .map_err(|error| custody(path, error))
}

/// Decode `bytes` as canonical Norito JSON of `T` (re-encoding must reproduce them).
fn decode_canonical<T: json::JsonDeserialize + json::JsonSerialize>(
    path: &Path,
    bytes: &[u8],
) -> Result<T, JournalError> {
    let value: T =
        json::from_slice(bytes).map_err(|_| JournalError::Malformed(path.to_path_buf()))?;
    let canonical =
        json::to_vec(&value).map_err(|_| JournalError::Malformed(path.to_path_buf()))?;
    if canonical != bytes {
        return Err(JournalError::Malformed(path.to_path_buf()));
    }
    Ok(value)
}

/// The journal root (`journal_dir` of the `[sccp]` client config).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpJournalRoot {
    root: PathBuf,
    directory: RetainedDirectory,
}

impl SccpJournalRoot {
    /// Open the root, creating it owner-only if missing (its parent must exist).
    ///
    /// # Errors
    ///
    /// Returns [`JournalError::Unsafe`] for a symlink or a directory with group or other
    /// permissions, or an I/O error.
    pub fn open(root: &Path) -> Result<Self, JournalError> {
        let directory = match iroha_fs::PrivateDirectory::open(root) {
            Ok(directory) => directory,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Preserve the parent-exists contract. No missing ancestor is created,
                // and an existing directory's custody is never repaired implicitly.
                let parent = root
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                let name = root
                    .file_name()
                    .ok_or_else(|| JournalError::Unsafe(root.to_owned()))?;
                let parent =
                    iroha_fs::OwnerDirectory::open(parent).map_err(|error| custody(root, error))?;
                parent
                    .create_private_child(name)
                    .map_err(|error| custody(root, error))?
            }
            Err(error) => return Err(custody(root, error)),
        };
        Ok(Self {
            root: directory.path().to_owned(),
            directory: RetainedDirectory::new(directory)?,
        })
    }

    /// The retained absolute root directory, including when opened through a relative path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// The journal of one Taira network.
    ///
    /// # Errors
    ///
    /// See [`Self::open`].
    pub fn network(&self, network_id: &NetworkId) -> Result<SccpNetworkJournal, JournalError> {
        let network_id = *network_id.as_bytes();
        let name = hex::encode(network_id);
        let directory = self
            .directory
            .ensure_child(&name)
            .map_err(|error| custody(&self.root.join(name), error))?;
        Ok(SccpNetworkJournal {
            dir: directory.path().to_owned(),
            network_id,
            directory: RetainedDirectory::new(directory)?,
        })
    }
}

/// The journal of one Taira `NetworkId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpNetworkJournal {
    dir: PathBuf,
    network_id: [u8; 32],
    directory: RetainedDirectory,
}

impl SccpNetworkJournal {
    /// The Taira `NetworkId` bytes.
    #[must_use]
    pub fn network_id(&self) -> [u8; 32] {
        self.network_id
    }

    /// Open (creating if missing) the journal of `flow`.
    ///
    /// # Errors
    ///
    /// See [`SccpJournalRoot::open`].
    pub fn flow(&self, flow: &SccpFlowIdV1) -> Result<SccpFlowJournal, JournalError> {
        let name = flow.dir_name();
        let directory = self
            .directory
            .ensure_child(&name)
            .map_err(|error| custody(&self.dir.join(name), error))?;
        Ok(SccpFlowJournal {
            dir: directory.path().to_owned(),
            network_id: self.network_id,
            flow: *flow,
            directory: RetainedDirectory::new(directory)?,
        })
    }

    /// Every flow journaled for this network, sorted.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or [`JournalError::Inconsistent`] for a foreign entry.
    pub fn flows(&self) -> Result<Vec<SccpFlowIdV1>, JournalError> {
        let mut flows = Vec::new();
        for name in self
            .directory
            .entries(usize::MAX)
            .map_err(io("list flows"))?
        {
            let flow = name
                .to_str()
                .and_then(SccpFlowIdV1::from_dir_name)
                .ok_or_else(|| {
                    JournalError::Inconsistent(format!(
                        "unexpected entry `{}` in the network journal",
                        name.to_string_lossy()
                    ))
                })?;
            flows.push(flow);
        }
        flows.sort_unstable();
        Ok(flows)
    }
}

/// The journal of one flow: dense, append-only steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpFlowJournal {
    dir: PathBuf,
    network_id: [u8; 32],
    flow: SccpFlowIdV1,
    directory: RetainedDirectory,
}

impl SccpFlowJournal {
    /// The flow.
    #[must_use]
    pub fn flow(&self) -> SccpFlowIdV1 {
        self.flow
    }

    fn step_dir(&self, index: u32) -> PathBuf {
        self.dir.join(format!("{STEP_DIR_PREFIX}{index:06}"))
    }

    fn check_record(&self, record: &SccpJournalRecordV1, index: u32) -> Result<(), JournalError> {
        if record.network_id != self.network_id || record.flow != self.flow || record.step != index
        {
            return Err(JournalError::Inconsistent(format!(
                "step {index} does not belong to flow {} of this network",
                self.flow.dir_name()
            )));
        }
        Ok(())
    }

    /// Number of steps on disk (steps must be dense from 0).
    ///
    /// # Errors
    ///
    /// Returns an I/O error or [`JournalError::Inconsistent`].
    pub fn step_count(&self) -> Result<u32, JournalError> {
        let mut indexes = Vec::new();
        for name in self
            .directory
            .entries(MAX_STEPS as usize + 1)
            .map_err(io("list steps"))?
        {
            let index = name
                .to_str()
                .and_then(|name| name.strip_prefix(STEP_DIR_PREFIX))
                .filter(|digits| digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|digits| digits.parse::<u32>().ok())
                .ok_or_else(|| {
                    JournalError::Inconsistent(format!(
                        "unexpected entry `{}` in the flow journal",
                        name.to_string_lossy()
                    ))
                })?;
            indexes.push(index);
        }
        indexes.sort_unstable();
        for (expected, index) in (0_u32..).zip(&indexes) {
            if *index != expected {
                return Err(JournalError::Inconsistent(format!(
                    "flow {} misses step {expected}",
                    self.flow.dir_name()
                )));
            }
        }
        u32::try_from(indexes.len())
            .map_err(|_| JournalError::Inconsistent("too many steps".to_owned()))
    }

    /// Read one step without taking its lock. A step directory without `operation.json` (and
    /// without later records) is [`SccpStepStateV1::Empty`].
    ///
    /// # Errors
    ///
    /// Returns an I/O, safety, consistency or format error, including for a step that holds a
    /// submission marker or an outcome but no prepared record.
    pub fn status(&self, index: u32) -> Result<SccpStepStatusV1, JournalError> {
        let dir = self.step_dir(index);
        let directory = self
            .directory
            .open_child(dir.file_name().expect("step name"))
            .map_err(|error| custody(&dir, error))?;
        // Records are installed in the order operation, submission, outcome and never removed,
        // so reading them in reverse order never sees a later record without its predecessors
        // even while another process advances the step.
        let applied = dir.join(APPLIED_FILE);
        let outcome = read_private_file(&directory, APPLIED_FILE)?
            .map(|bytes| decode_canonical::<SccpJournalOutcomeV1>(&applied, &bytes))
            .transpose()?;
        let submitted = read_private_file(&directory, SUBMISSION_FILE)?.is_some();
        let operation = dir.join(OPERATION_FILE);
        let Some(bytes) = read_private_file(&directory, OPERATION_FILE)? else {
            if outcome.is_some() || submitted {
                return Err(JournalError::Inconsistent(format!(
                    "step {index} has progress records but no prepared record"
                )));
            }
            return Ok(SccpStepStatusV1 {
                index,
                record: None,
                state: SccpStepStateV1::Empty,
            });
        };
        let record: SccpJournalRecordV1 = decode_canonical(&operation, &bytes)?;
        self.check_record(&record, index)?;
        let state = match outcome {
            Some(outcome) => SccpStepStateV1::Completed(outcome.outcome),
            None if submitted => SccpStepStateV1::Submitted,
            None => SccpStepStateV1::Prepared,
        };
        Ok(SccpStepStatusV1 {
            index,
            record: Some(record),
            state,
        })
    }

    /// Every step in order.
    ///
    /// # Errors
    ///
    /// See [`Self::status`].
    pub fn steps(&self) -> Result<Vec<SccpStepStatusV1>, JournalError> {
        (0..self.step_count()?)
            .map(|index| self.status(index))
            .collect()
    }

    /// The step to resume: the last step when it has no outcome yet. An
    /// [`SccpStepStateV1::Empty`] step is resumed by appending its record again.
    ///
    /// # Errors
    ///
    /// See [`Self::status`].
    pub fn resume_point(&self) -> Result<Option<SccpStepStatusV1>, JournalError> {
        let count = self.step_count()?;
        let Some(last) = count.checked_sub(1) else {
            return Ok(None);
        };
        let status = self.status(last)?;
        Ok(match status.state {
            SccpStepStateV1::Completed(_) => None,
            _ => Some(status),
        })
    }

    /// Journal a step and hold its lock. `record.step` must be the next index after a completed
    /// step, or the index of the last step when that step is [`SccpStepStateV1::Empty`] (which
    /// is then reclaimed); the record must name this network and flow.
    ///
    /// # Errors
    ///
    /// Returns [`JournalError::Inconsistent`] or a write error.
    pub fn append(&self, record: &SccpJournalRecordV1) -> Result<SccpJournalStep, JournalError> {
        let count = self.step_count()?;
        if let Some(last) = count.checked_sub(1) {
            match self.status(last)?.state {
                SccpStepStateV1::Completed(_) => {}
                SccpStepStateV1::Empty => {
                    if record.step != last {
                        return Err(JournalError::Inconsistent(format!(
                            "step {last} was never prepared; journal it again first"
                        )));
                    }
                    self.check_record(record, last)?;
                    return SccpJournalStep::install(self.reclaim_empty_step(last)?, record);
                }
                SccpStepStateV1::Prepared | SccpStepStateV1::Submitted => {
                    return Err(JournalError::Inconsistent(format!(
                        "step {last} has no outcome yet; resume it first"
                    )));
                }
            }
        }
        if count > MAX_STEPS {
            return Err(JournalError::Inconsistent("too many steps".to_owned()));
        }
        self.check_record(record, count)?;
        SccpJournalStep::install(
            Journal::create_child(&self.directory, format!("{STEP_DIR_PREFIX}{count:06}"))
                .map_err(storage)?,
            record,
        )
    }

    /// Lock the [`SccpStepStateV1::Empty`] step `index` for a new prepared record.
    ///
    /// `Journal::create` makes the directory, then its lock, then nothing else until the record
    /// is written. With the lock present the step is reopened under it, which also refuses a
    /// step another process holds. Without it, nothing was ever written: the directory is removed
    /// (`rmdir` fails on anything it holds) and created afresh.
    fn reclaim_empty_step(&self, index: u32) -> Result<Journal, JournalError> {
        let dir = self.step_dir(index);
        let name = dir.file_name().expect("step name");
        let directory = self
            .directory
            .open_child(name)
            .map_err(|error| custody(&dir, error))?;
        if directory
            .entries(usize::MAX)
            .map_err(|error| custody(&dir, error))?
            .iter()
            .any(|name| name == LOCK_FILE)
        {
            Journal::open_optional_child(&self.directory, name)
                .map_err(storage)?
                .ok_or_else(|| {
                    JournalError::Inconsistent("step disappeared before locking".to_owned())
                })
        } else {
            directory
                .remove_empty()
                .map_err(io("remove an unprepared step"))?;
            Journal::create_child(&self.directory, name).map_err(storage)
        }
    }

    /// Open an existing prepared step and hold its exclusive lock.
    ///
    /// # Errors
    ///
    /// Returns an operation-journal, consistency or format error; an [`SccpStepStateV1::Empty`] step
    /// is refused (append its record again instead).
    pub fn open_step(&self, index: u32) -> Result<SccpJournalStep, JournalError> {
        if self.status(index)?.state == SccpStepStateV1::Empty {
            return Err(JournalError::Inconsistent(format!(
                "step {index} was never prepared; journal it again first"
            )));
        }
        let journal =
            Journal::open_optional_child(&self.directory, format!("{STEP_DIR_PREFIX}{index:06}"))
                .map_err(storage)?
                .ok_or_else(|| {
                    JournalError::Inconsistent(
                        "prepared step disappeared before locking".to_owned(),
                    )
                })?;
        let record: SccpJournalRecordV1 = journal.read_operation().map_err(storage)?;
        self.check_record(&record, index)?;
        Ok(SccpJournalStep { journal, record })
    }
}

/// One locked step.
#[derive(Debug)]
pub struct SccpJournalStep {
    journal: Journal,
    record: SccpJournalRecordV1,
}

#[derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct DiscoveryHint {
    original: [u8; 32],
    value: Vec<u8>,
}

impl SccpJournalStep {
    /// Read the retained final outcome, if one was durably published.
    ///
    /// # Errors
    /// Refuses unsafe, noncanonical or unreadable stored evidence.
    pub fn outcome(&self) -> Result<Option<SccpJournalOutcomeV1>, JournalError> {
        let original: SccpJournalRecordV1 = self.journal.read_operation().map_err(storage)?;
        if original != self.record {
            return Err(JournalError::Inconsistent(
                "the retained original operation changed".to_owned(),
            ));
        }
        let outcome = self.journal.read_applied_evidence().map_err(storage)?;
        let after: SccpJournalRecordV1 = self.journal.read_operation().map_err(storage)?;
        if after != original {
            return Err(JournalError::Inconsistent(
                "the retained original operation changed".to_owned(),
            ));
        }
        Ok(outcome)
    }

    /// Read a resumable, non-authoritative discovery cursor for this exact original step.
    /// Corrupt content or a hint copied from another operation safely restarts discovery.
    ///
    /// # Errors
    /// Refuses unsafe original storage custody or changed prepared bytes.
    pub fn discovery<T: json::JsonDeserialize + json::JsonSerialize>(
        &self,
    ) -> Result<Option<T>, JournalError> {
        let original: SccpJournalRecordV1 = self.journal.read_operation().map_err(storage)?;
        if original != self.record {
            return Err(JournalError::Inconsistent("original changed".to_owned()));
        }
        let Some(hint) = self
            .journal
            .read_discovery_hint::<DiscoveryHint>()
            .map_err(storage)?
        else {
            return Ok(None);
        };
        let bytes = json::to_vec(&original)
            .map_err(|_| JournalError::Malformed(self.journal.path().to_owned()))?;
        if hint.original != iroha_sccp::v1::hashes::keccak256(&[&bytes]) {
            return Ok(None);
        }
        Ok(json::from_slice::<T>(&hint.value)
            .ok()
            .filter(|value| json::to_vec(value).ok().as_deref() == Some(hint.value.as_slice())))
    }

    /// Persist a bounded hint for discovery only; it cannot replace original bytes or outcomes.
    ///
    /// # Errors
    /// Refuses unsafe custody, oversized hints, serialization or durability failure.
    pub fn save_discovery<T: json::JsonSerialize>(&self, value: &T) -> Result<(), JournalError> {
        let original: SccpJournalRecordV1 = self.journal.read_operation().map_err(storage)?;
        if original != self.record {
            return Err(JournalError::Inconsistent("original changed".to_owned()));
        }
        let bytes = json::to_vec(&original)
            .map_err(|_| JournalError::Malformed(self.journal.path().to_owned()))?;
        let value = json::to_vec(value)
            .map_err(|_| JournalError::Malformed(self.journal.path().to_owned()))?;
        self.journal
            .write_discovery_hint(&DiscoveryHint {
                original: iroha_sccp::v1::hashes::keccak256(&[&bytes]),
                value,
            })
            .map_err(storage)
    }
    /// Install `record` as the prepared record of the locked step `journal`.
    fn install(journal: Journal, record: &SccpJournalRecordV1) -> Result<Self, JournalError> {
        journal.write_operation(record).map_err(storage)?;
        Ok(Self {
            journal,
            record: record.clone(),
        })
    }

    /// The prepared record.
    #[must_use]
    pub fn record(&self) -> &SccpJournalRecordV1 {
        &self.record
    }

    /// Revalidate the stored canonical operation on both sides of a journal action.
    fn with_original<T>(
        &self,
        action: impl FnOnce() -> Result<T, JournalError>,
    ) -> Result<T, JournalError> {
        self.require_original()?;
        let result = action();
        self.require_original()?;
        result
    }

    /// Require the current private operation record to equal the prepared record.
    fn require_original(&self) -> Result<(), JournalError> {
        let original: SccpJournalRecordV1 = self.journal.read_operation().map_err(storage)?;
        if original != self.record {
            return Err(JournalError::Inconsistent(
                "the retained original operation changed".to_owned(),
            ));
        }
        Ok(())
    }

    /// Durably record the pre-dispatch marker. Only the call that returns `true` may submit;
    /// `false` means an earlier attempt exists and the step must be reconciled instead.
    ///
    /// # Errors
    ///
    /// Refuses an unsafe or changed stored operation and returns journal errors.
    pub fn begin_submission(&self) -> Result<bool, JournalError> {
        self.with_original(|| {
            self.journal
                .record_submission(&self.record)
                .map_err(storage)
        })
    }

    /// Whether the pre-dispatch marker exists.
    ///
    /// # Errors
    ///
    /// Refuses an unsafe or changed stored operation and returns journal errors.
    pub fn submission_recorded(&self) -> Result<bool, JournalError> {
        self.with_original(|| {
            self.journal
                .submission_recorded(&self.record)
                .map_err(storage)
        })
    }

    /// Record the step's outcome, once (an identical rewrite is accepted).
    ///
    /// # Errors
    ///
    /// Refuses an unsafe or changed stored operation and returns journal errors,
    /// including for a different outcome.
    pub fn complete(&self, outcome: &SccpJournalOutcomeV1) -> Result<(), JournalError> {
        self.with_original(|| {
            self.journal
                .write_applied_evidence(outcome)
                .map_err(storage)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as symlink;

    use super::*;

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([seed; 32]),
        ))
    }

    fn flow() -> SccpFlowIdV1 {
        SccpFlowIdV1 {
            kind: SccpFlowKindV1::Outbound,
            id: [0xab; 32],
        }
    }

    fn record(network: &SccpNetworkJournal, step: u32) -> SccpJournalRecordV1 {
        SccpJournalRecordV1 {
            network_id: network.network_id(),
            flow: flow(),
            step,
            chain: SccpNetworkV1::EthereumMainnet,
            action: crate::flow::SccpFlowActionV1::Finalize,
            submission: vec![0x02, 0xf8, 0x01],
            evidence: vec![SccpEvidenceItemV1 {
                label: "torii.message_proof".to_owned(),
                bytes: vec![1, 2, 3],
            }],
        }
    }

    fn outcome() -> SccpJournalOutcomeV1 {
        SccpJournalOutcomeV1 {
            outcome: SccpStepOutcomeKindV1::Confirmed,
            transaction: vec![0x55; 32],
            evidence: Vec::new(),
        }
    }

    fn root() -> (tempfile::TempDir, SccpJournalRoot) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = SccpJournalRoot::open(&dir.path().join("journal")).expect("root");
        (dir, root)
    }

    #[test]
    fn root_accepts_relative_and_existing_dot_paths_without_creating_missing_parents() {
        let working = tempfile::tempdir().unwrap();
        let mut child = iroha_fs::ProcessCommand::new(std::env::current_exe().unwrap()).unwrap();
        child
            .args([
                "--ignored",
                "--exact",
                "journal::tests::relative_root_child",
                "--nocapture",
            ])
            .env("IROHA_SCCP_RELATIVE_ROOT_CHILD", "1")
            .current_dir(working.path());
        let output = child.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("running 1 test"), "{stdout}");
        assert!(
            stdout.contains("relative-root-custody-complete"),
            "{stdout}"
        );
    }

    #[test]
    #[ignore = "invoked by the relative-root parent in its own local working directory"]
    fn relative_root_child() {
        assert_eq!(
            std::env::var("IROHA_SCCP_RELATIVE_ROOT_CHILD").as_deref(),
            Ok("1")
        );
        let current = std::env::current_dir().unwrap();
        let temporary = tempfile::tempdir_in(&current).unwrap();
        let relative = temporary
            .path()
            .strip_prefix(&current)
            .unwrap()
            .join("journal");
        let root = SccpJournalRoot::open(&relative).unwrap();
        assert!(root.path().is_absolute());
        let original = root.directory.identity;
        let reopened = SccpJournalRoot::open(&root.path().join(".")).unwrap();
        assert_eq!(reopened.directory.identity, original);
        assert_eq!(root.clone(), root);
        let missing = temporary.path().join("missing");
        assert!(SccpJournalRoot::open(&missing.join("journal")).is_err());
        assert!(!missing.exists());
        assert!(matches!(
            SccpJournalRoot::open(Path::new("")),
            Err(JournalError::Unsafe(_))
        ));
        assert!(SccpJournalRoot::open(&relative.join("..").join("other")).is_err());
        println!("relative-root-custody-complete");
    }

    fn assert_no_submission_or_outcome(step: &SccpJournalStep) {
        assert!(!step.journal.path().join(SUBMISSION_FILE).exists());
        assert!(!step.journal.path().join(APPLIED_FILE).exists());
    }

    fn assert_original_required(step: &SccpJournalStep) {
        assert!(step.begin_submission().is_err());
        assert!(step.submission_recorded().is_err());
        assert!(step.complete(&outcome()).is_err());
        assert_no_submission_or_outcome(step);
    }

    #[test]
    fn submission_and_completion_require_the_exact_stored_operation() {
        let (temporary, root) = root();
        let network = root.network(&network_id(0x11)).unwrap();
        let flow = network.flow(&flow()).unwrap();
        let record = record(&network, 0);
        let step = flow.append(&record).unwrap();
        let operation = step.journal.path().join(OPERATION_FILE);
        let original = json::to_vec(&record).unwrap();
        let mut changed = record.clone();
        changed.submission.push(0xff);
        fs::write(&operation, json::to_vec(&changed).unwrap()).unwrap();
        assert_original_required(&step);
        fs::write(&operation, &original).unwrap();
        let alias = temporary.path().join("operation-alias");
        fs::hard_link(&operation, &alias).unwrap();
        assert_original_required(&step);
        fs::remove_file(alias).unwrap();
        assert!(!step.submission_recorded().unwrap());
        assert!(step.begin_submission().unwrap());
        assert!(step.submission_recorded().unwrap());
        step.complete(&outcome()).unwrap();
        assert_eq!(step.outcome().unwrap(), Some(outcome()));
    }

    #[test]
    fn original_guard_revalidates_after_success_and_after_failed_actions() {
        let (_temporary, root) = root();
        let network = root.network(&network_id(0x11)).unwrap();
        let flow = network.flow(&flow()).unwrap();
        let record = record(&network, 0);
        let step = flow.append(&record).unwrap();
        let operation = step.journal.path().join(OPERATION_FILE);
        let original = json::to_vec(&record).unwrap();
        let mut changed = record.clone();
        changed.submission.push(0xff);
        for action_fails in [false, true] {
            let result = step.with_original(|| {
                fs::write(&operation, json::to_vec(&changed).unwrap()).unwrap();
                if action_fails {
                    Err(JournalError::Malformed(operation.clone()))
                } else {
                    Ok(())
                }
            });
            assert!(matches!(result, Err(JournalError::Inconsistent(_))));
            assert_no_submission_or_outcome(&step);
            fs::write(&operation, &original).unwrap();
        }
        assert!(matches!(
            step.with_original::<()>(|| Err(JournalError::Malformed(operation.clone()))),
            Err(JournalError::Malformed(_))
        ));
        assert_no_submission_or_outcome(&step);
    }

    #[test]
    fn step_retains_original_ancestry_after_all_selectors_are_dropped() {
        let (_temporary, root) = root();
        let network = root.network(&network_id(0x11)).unwrap();
        let flow = network.flow(&flow()).unwrap();
        let record = record(&network, 0);
        let step = flow.append(&record).unwrap();
        drop(flow);
        drop(network);
        drop(root);
        assert_eq!(step.record(), &record);
        assert!(step.begin_submission().unwrap());
        step.complete(&outcome()).unwrap();
        assert_eq!(step.outcome().unwrap(), Some(outcome()));
    }

    #[test]
    fn malformed_and_unsafe_records_remain_distinct_from_initial_absence() {
        let (_temporary, root) = root();
        let network = root.network(&network_id(0x11)).unwrap();
        let flow = network.flow(&flow()).unwrap();
        let directory = flow.directory.create_child("step-000000").unwrap();
        assert!(
            read_private_file(&directory, OPERATION_FILE)
                .unwrap()
                .is_none()
        );
        let mut large = directory
            .create_retained_private(OPERATION_FILE, MAX_RECORD_BYTES as usize + 2)
            .unwrap();
        use std::io::Write as _;
        large
            .write_all(&vec![0; MAX_RECORD_BYTES as usize + 1])
            .unwrap();
        drop(large);
        assert!(matches!(
            read_private_file(&directory, OPERATION_FILE),
            Err(JournalError::Malformed(_))
        ));
        fs::remove_file(directory.path().join(OPERATION_FILE)).unwrap();
        let wrong_type = directory.create_child(OPERATION_FILE).unwrap();
        assert!(matches!(
            read_private_file(&directory, OPERATION_FILE),
            Err(JournalError::Unsafe(_))
        ));
        drop(wrong_type);
    }

    #[cfg(windows)]
    fn change_everyone_read(path: &Path, grant: bool) {
        assert!(
            std::process::Command::new("icacls")
                .arg(path)
                .arg(if grant { "/grant" } else { "/remove:g" })
                .arg(if grant { "*S-1-1-0:R" } else { "*S-1-1-0" })
                .status()
                .unwrap()
                .success()
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_journal_rejects_inherited_custody_and_live_acl_or_link_changes() {
        let temporary = tempfile::tempdir().unwrap();
        assert!(matches!(
            SccpJournalRoot::open(temporary.path()),
            Err(JournalError::Unsafe(_))
        ));
        let root = SccpJournalRoot::open(&temporary.path().join("private")).unwrap();
        let network = root.network(&network_id(0x11)).unwrap();
        let flow = network.flow(&flow()).unwrap();
        let step = flow.append(&record(&network, 0)).unwrap();
        let operation = step.journal.path().join(OPERATION_FILE);
        for path in [&operation, root.path()] {
            change_everyone_read(path, true);
            assert!(flow.status(0).is_err());
            assert_original_required(&step);
            change_everyone_read(path, false);
        }
        assert_eq!(flow.status(0).unwrap().state, SccpStepStateV1::Prepared);
        let alias = temporary.path().join("operation-alias");
        fs::hard_link(&operation, &alias).unwrap();
        assert!(matches!(flow.status(0), Err(JournalError::Unsafe(_))));
        assert_original_required(&step);
        fs::remove_file(alias).unwrap();
        assert_eq!(flow.status(0).unwrap().state, SccpStepStateV1::Prepared);
        assert!(fs::rename(root.path(), temporary.path().join("displaced")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_journal_refuses_junction_networks_without_following_target() {
        let (temporary, root) = root();
        let target =
            iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("target")).unwrap();
        let network_id = network_id(0x11);
        let junction = root.path().join(hex::encode(network_id.as_bytes()));
        assert!(
            std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(&junction)
                .arg(target.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(matches!(
            root.network(&network_id),
            Err(JournalError::Unsafe(_))
        ));
        assert!(target.entries(1).unwrap().is_empty());
    }

    #[test]
    fn flow_ids_roundtrip_through_directory_names() {
        let id = flow();
        let name = id.dir_name();
        assert_eq!(name, format!("outbound-{}", "ab".repeat(32)));
        assert_eq!(SccpFlowIdV1::from_dir_name(&name), Some(id));
        for kind in SccpFlowKindV1::ALL {
            let flow = SccpFlowIdV1 { kind, id: [7; 32] };
            assert_eq!(SccpFlowIdV1::from_dir_name(&flow.dir_name()), Some(flow));
        }
        assert_eq!(SccpFlowIdV1::from_dir_name("outbound-00"), None);
        assert_eq!(
            SccpFlowIdV1::from_dir_name(&format!("mint-{}", "ab".repeat(32))),
            None
        );
        assert_eq!(
            SccpFlowIdV1::from_dir_name(&format!("outbound-{}", "AB".repeat(32))),
            None
        );
    }

    #[test]
    fn committee_sync_kind_roundtrips_and_rejects_retired_roster_kind() {
        let kind = SccpFlowKindV1::CommitteeSync;
        assert_eq!(kind.as_str(), "committee_sync");
        assert_eq!(SccpFlowKindV1::from_label("committee_sync"), Some(kind));
        let bytes = json::to_vec(&kind).expect("current kind encodes");
        assert_eq!(
            json::from_slice::<SccpFlowKindV1>(&bytes).expect("current kind decodes"),
            kind
        );
        let text = String::from_utf8(bytes).expect("UTF-8 JSON");
        assert!(text.contains("committee_sync"), "{text}");
        let retired = text.replace("committee_sync", "roster_sync");
        assert!(json::from_str::<SccpFlowKindV1>(&retired).is_err());
        assert_eq!(SccpFlowKindV1::from_label("roster_sync"), None);
        assert_eq!(
            SccpFlowIdV1::from_dir_name(&format!("roster_sync-{}", "ab".repeat(32))),
            None
        );
    }

    #[test]
    fn committee_sync_step_reopens_with_current_identity_without_resubmission() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let mut record = record(&network, 0);
        record.flow.kind = SccpFlowKindV1::CommitteeSync;
        record.action = crate::flow::SccpFlowActionV1::Checkpoint;
        record.evidence[0].label = "evm.committee_state".to_owned();
        let bytes = json::to_vec(&record).expect("current record encodes");
        assert_eq!(
            decode_canonical::<SccpJournalRecordV1>(Path::new("operation.json"), &bytes)
                .expect("current record decodes"),
            record
        );
        let text = String::from_utf8(bytes).expect("UTF-8 JSON");
        let retired = text.replace("committee_sync", "roster_sync");
        assert!(json::from_str::<SccpJournalRecordV1>(&retired).is_err());
        assert_eq!(
            record.flow.dir_name(),
            format!("committee_sync-{}", "ab".repeat(32))
        );
        let journal = network.flow(&record.flow).expect("committee flow");
        let step = journal.append(&record).expect("prepared checkpoint step");
        assert!(step.begin_submission().expect("one original submission"));
        drop(step);
        drop(journal);

        let reopened = network.flow(&record.flow).expect("reopened committee flow");
        let resumed = reopened.resume_point().expect("resume").expect("pending");
        assert_eq!(resumed.state, SccpStepStateV1::Submitted);
        let step = reopened.open_step(0).expect("original checkpoint step");
        assert_eq!(step.record(), &record);
        assert!(!step.begin_submission().expect("no duplicate submission"));
        step.complete(&outcome()).expect("original outcome");
        drop(step);
        assert_eq!(reopened.resume_point().expect("completed"), None);
        assert_eq!(network.flows().expect("current flows"), vec![record.flow]);
    }

    #[test]
    fn discovery_hint_resumes_bounded_pages_but_corruption_never_changes_original_custody() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).unwrap();
        let original = record(&network, 0);
        let flow = network.flow(&original.flow).unwrap();
        let step = flow.append(&original).unwrap();
        assert_eq!(step.discovery::<u64>().unwrap(), None);
        step.save_discovery(&91_u64).unwrap();
        step.save_discovery(&75_u64).unwrap();
        assert!(step.begin_submission().unwrap());
        let path = step.journal.path().join("discovery.json");
        let original_bytes = fs::read(step.journal.path().join(OPERATION_FILE)).unwrap();
        drop(step);
        let step = flow.open_step(0).unwrap();
        assert_eq!(step.discovery::<u64>().unwrap(), Some(75));
        assert!(!step.begin_submission().unwrap());
        assert!(step.outcome().unwrap().is_none());
        fs::write(&path, b"incomplete hint").unwrap();
        assert_eq!(step.discovery::<u64>().unwrap(), None);
        step.save_discovery(&64_u64).unwrap();
        let foreign = DiscoveryHint {
            original: [9; 32],
            value: json::to_vec(&63_u64).unwrap(),
        };
        fs::write(&path, json::to_vec(&foreign).unwrap()).unwrap();
        assert_eq!(step.discovery::<u64>().unwrap(), None);
        assert!(step.save_discovery(&"x".repeat(4096)).is_err());
        assert_eq!(
            fs::read(step.journal.path().join(OPERATION_FILE)).unwrap(),
            original_bytes
        );
        assert_eq!(step.record(), &original);
        assert!(step.outcome().unwrap().is_none());
        assert!(!step.begin_submission().unwrap());
        fs::remove_file(&path).unwrap();
        symlink("operation.json", &path).unwrap();
        assert!(
            step.discovery::<u64>().is_err(),
            "unsafe custody is never absent discovery"
        );
    }

    #[test]
    fn records_roundtrip_as_canonical_json() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let record = record(&network, 0);
        let bytes = json::to_vec(&record).expect("json");
        let path = Path::new("operation.json");
        assert_eq!(
            decode_canonical::<SccpJournalRecordV1>(path, &bytes).expect("decodes"),
            record
        );
        let mut spaced = b" ".to_vec();
        spaced.extend_from_slice(&bytes);
        assert_eq!(
            decode_canonical::<SccpJournalRecordV1>(path, &spaced),
            Err(JournalError::Malformed(path.to_path_buf()))
        );
        let text = String::from_utf8(bytes).expect("utf8");
        assert!(text.contains("\"submission\":\"AvgB\""), "{text}");
    }

    #[test]
    fn steps_are_dense_resumable_and_written_once() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let flow_journal = network.flow(&flow()).expect("flow");
        assert_eq!(flow_journal.flow(), flow());
        assert_eq!(flow_journal.resume_point().expect("empty"), None);

        let step = flow_journal.append(&record(&network, 0)).expect("append");
        assert_eq!(step.record().step, 0);
        assert!(
            flow_journal.append(&record(&network, 1)).is_err(),
            "step 0 is open"
        );
        assert!(!step.submission_recorded().expect("marker"));
        assert!(step.begin_submission().expect("first dispatch"));
        assert!(!step.begin_submission().expect("no second dispatch"));
        drop(step);

        let resume = flow_journal
            .resume_point()
            .expect("resume")
            .expect("pending");
        assert_eq!(resume.state, SccpStepStateV1::Submitted);
        let step = flow_journal.open_step(0).expect("reopen");
        step.complete(&outcome()).expect("outcome");
        step.complete(&outcome()).expect("identical rewrite");
        let mut other = outcome();
        other.outcome = SccpStepOutcomeKindV1::Failed;
        assert!(step.complete(&other).is_err(), "outcomes are written once");
        drop(step);
        assert_eq!(flow_journal.resume_point().expect("done"), None);

        assert!(
            flow_journal.append(&record(&network, 2)).is_err(),
            "indexes are dense"
        );
        let second = flow_journal.append(&record(&network, 1)).expect("step 1");
        drop(second);
        let steps = flow_journal.steps().expect("steps");
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0].state,
            SccpStepStateV1::Completed(SccpStepOutcomeKindV1::Confirmed)
        );
        assert_eq!(steps[1].state, SccpStepStateV1::Prepared);
        assert_eq!(network.flows().expect("flows"), vec![flow()]);
    }

    #[test]
    fn an_unprepared_step_is_reported_and_reclaimed() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let flow_journal = network.flow(&flow()).expect("flow");
        let step_dir = |index: u32| flow_journal.step_dir(index);

        // A crash after `mkdirat`, before the lock: an empty directory.
        create_private_dir(&step_dir(0)).expect("empty step");
        let empty = SccpStepStatusV1 {
            index: 0,
            record: None,
            state: SccpStepStateV1::Empty,
        };
        assert_eq!(flow_journal.step_count().expect("count"), 1);
        assert_eq!(flow_journal.status(0).expect("status"), empty);
        assert_eq!(flow_journal.steps().expect("steps"), vec![empty.clone()]);
        assert_eq!(
            flow_journal.resume_point().expect("resume"),
            Some(empty.clone())
        );
        assert!(matches!(
            flow_journal.open_step(0),
            Err(JournalError::Inconsistent(_))
        ));
        assert!(matches!(
            flow_journal.append(&record(&network, 1)),
            Err(JournalError::Inconsistent(_))
        ));
        let step = flow_journal
            .append(&record(&network, 0))
            .expect("reclaimed");
        assert!(step.begin_submission().expect("first dispatch"));
        step.complete(&outcome()).expect("outcome");
        drop(step);
        assert_eq!(flow_journal.resume_point().expect("done"), None);

        // A crash after the lock, before `operation.json`: reopened under its lock.
        drop(Journal::create(&step_dir(1)).expect("locked step"));
        let pending = flow_journal
            .resume_point()
            .expect("resume")
            .expect("pending");
        assert_eq!((pending.index, pending.state), (1, SccpStepStateV1::Empty));
        let held = Journal::open(&step_dir(1)).expect("another holder");
        assert!(
            matches!(
                flow_journal.append(&record(&network, 1)),
                Err(JournalError::Storage(_))
            ),
            "a step another process holds is not reclaimed"
        );
        drop(held);
        let step = flow_journal
            .append(&record(&network, 1))
            .expect("reclaimed");
        assert_eq!(step.record().step, 1);
        drop(step);
        assert_eq!(
            flow_journal.status(1).expect("status").state,
            SccpStepStateV1::Prepared
        );
        assert_eq!(
            flow_journal.status(1).expect("status").record,
            Some(record(&network, 1))
        );
    }

    #[test]
    fn an_unprepared_step_is_never_emptied_by_force() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let flow_journal = network.flow(&flow()).expect("flow");
        let dir = flow_journal.step_dir(0);
        create_private_dir(&dir).expect("empty step");
        fs::write(dir.join("stray"), b"x").expect("stray");
        assert_eq!(
            flow_journal.status(0).expect("status").state,
            SccpStepStateV1::Empty
        );
        assert!(matches!(
            flow_journal.append(&record(&network, 0)),
            Err(JournalError::Io { .. })
        ));
        assert!(dir.join("stray").exists(), "unknown content is kept");

        // Progress records without a prepared record are not an empty step.
        let marker = flow_journal.step_dir(0).join(SUBMISSION_FILE);
        iroha_fs::PrivateDirectory::open(marker.parent().unwrap())
            .unwrap()
            .write_atomic(SUBMISSION_FILE, b"{}", iroha_fs::PublishMode::CreateNew)
            .expect("private marker");
        assert!(matches!(
            flow_journal.status(0),
            Err(JournalError::Inconsistent(_))
        ));
        assert!(flow_journal.resume_point().is_err());
    }

    #[test]
    fn records_are_bound_to_their_network_and_flow() {
        let (_dir, root) = root();
        let network = root.network(&network_id(0x11)).expect("network");
        let reset = root.network(&network_id(0x12)).expect("reset network");
        let flow_journal = network.flow(&flow()).expect("flow");
        assert!(matches!(
            flow_journal.append(&record(&reset, 0)),
            Err(JournalError::Inconsistent(_))
        ));
        let mut other_flow = record(&network, 0);
        other_flow.flow.kind = SccpFlowKindV1::Refund;
        assert!(flow_journal.append(&other_flow).is_err());
        assert_ne!(network.network_id(), reset.network_id());
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_directories_are_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let open = dir.path().join("open");
        fs::create_dir(&open).expect("mkdir");
        fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).expect("chmod");
        assert_eq!(
            SccpJournalRoot::open(&open),
            Err(JournalError::Unsafe(open.clone()))
        );
        let link = dir.path().join("link");
        let private = dir.path().join("private");
        fs::create_dir(&private).expect("mkdir");
        fs::set_permissions(&private, fs::Permissions::from_mode(0o700)).expect("chmod");
        symlink(&private, &link).expect("symlink");
        assert_eq!(
            SccpJournalRoot::open(&link),
            Err(JournalError::Unsafe(link.clone()))
        );
        let root = SccpJournalRoot::open(&private).expect("private root");
        assert_eq!(root.path(), private);
        let network = root.network(&network_id(0x11)).expect("network");
        fs::write(private.join(hex::encode([0x11; 32])).join("stray"), b"x").expect("stray");
        assert!(matches!(
            network.flows(),
            Err(JournalError::Inconsistent(_))
        ));
    }
}
