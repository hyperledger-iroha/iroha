//! Client side of the feature-isolated real-network consensus message controller.

use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use color_eyre::eyre::{Result, eyre};
use iroha_crypto::{Hash as CryptoHash, HashOf};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{
            BlockSubject, ExecutionCommitment, MAX_VALIDATORS_PER_HEIGHT, ValidatorIndex,
        },
    },
    peer::PeerId,
};
use norito::json::{Map, Value};
use tokio::time::sleep;

pub(crate) const CONTROL_DIR_ENV: &str = "IROHA_TEST_CONSENSUS_MESSAGE_CONTROL_DIR";
const CONTROL_FILE: &str = "command.norito.json";
const ACK_FILE: &str = "ack.norito.json";
const NATIVE_AMX_FAULT_COMMAND_FILE: &str = "native-amx-fault-command.norito.json";
const NATIVE_AMX_FAULT_ACK_FILE: &str = "native-amx-fault-ack.norito.json";
const NATIVE_AMX_FAULT_FORMAT_VERSION: u64 = 1;
const FORMAT_VERSION: u64 = 4;
const MAX_CONTROL_BYTES: usize = 64 * 1024;
const MAX_ACK_BYTES: usize = 1024 * 1024;
const MAX_RULES: usize = 256;
const MAX_HOLDS: usize = 1_024;
const MAX_HELD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RELEASES: usize = MAX_HOLDS;
const DEFAULT_QUEUE_CAPACITY: usize = 512;
const ACK_POLL: Duration = Duration::from_millis(10);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Exact authoritative Sumeragi v2 payload kind matched by a receiver-local rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConsensusMessageControlKind {
    /// Signed proposal.
    Proposal,
    /// Prepare vote.
    PrepareVote,
    /// Commit vote.
    CommitVote,
    /// Prepare quorum certificate.
    PrepareCertificate,
    /// Commit quorum certificate.
    CommitCertificate,
    /// Timeout vote.
    TimeoutVote,
    /// Timeout certificate.
    TimeoutCertificate,
    /// Payload manifest.
    PayloadManifest,
    /// Payload chunk. Chunks have no directly encoded height/view and therefore
    /// appear only in drain descriptors, never in exact round rules.
    PayloadChunk,
    /// Certified-body request.
    CertifiedBodyRequest,
    /// Certified-body response.
    CertifiedBodyResponse,
    /// Commit-certificate request.
    CommitCertificateRequest,
    /// Commit-certificate response.
    CommitCertificateResponse,
}

impl ConsensusMessageControlKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::PrepareVote => "prepare_vote",
            Self::CommitVote => "commit_vote",
            Self::PrepareCertificate => "prepare_certificate",
            Self::CommitCertificate => "commit_certificate",
            Self::TimeoutVote => "timeout_vote",
            Self::TimeoutCertificate => "timeout_certificate",
            Self::PayloadManifest => "payload_manifest",
            Self::PayloadChunk => "payload_chunk",
            Self::CertifiedBodyRequest => "certified_body_request",
            Self::CertifiedBodyResponse => "certified_body_response",
            Self::CommitCertificateRequest => "commit_certificate_request",
            Self::CommitCertificateResponse => "commit_certificate_response",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "proposal" => Ok(Self::Proposal),
            "prepare_vote" => Ok(Self::PrepareVote),
            "commit_vote" => Ok(Self::CommitVote),
            "prepare_certificate" => Ok(Self::PrepareCertificate),
            "commit_certificate" => Ok(Self::CommitCertificate),
            "timeout_vote" => Ok(Self::TimeoutVote),
            "timeout_certificate" => Ok(Self::TimeoutCertificate),
            "payload_manifest" => Ok(Self::PayloadManifest),
            "payload_chunk" => Ok(Self::PayloadChunk),
            "certified_body_request" => Ok(Self::CertifiedBodyRequest),
            "certified_body_response" => Ok(Self::CertifiedBodyResponse),
            "commit_certificate_request" => Ok(Self::CommitCertificateRequest),
            "commit_certificate_response" => Ok(Self::CommitCertificateResponse),
            _ => Err(eyre!("unknown consensus message-control kind `{value}`")),
        }
    }

    const fn has_exact_round(self) -> bool {
        !matches!(self, Self::PayloadChunk | Self::CommitCertificateRequest)
    }
}

/// Action taken when a rule matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsensusMessageControlAction {
    /// Discard the authenticated message.
    Drop,
    /// Retain the authenticated message in the receiver's bounded queue.
    Hold,
}

/// Exact feature-isolated Native AMX process-cut phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeAmxFaultPhase {
    /// Abort after authenticating and aggregating the participant PrepareQC.
    AfterPrepareQc,
    /// Abort after authenticating and aggregating the participant CommitQC.
    AfterCommitQc,
    /// Abort after constructing the exact State overlay and immediately before WSV publication.
    BeforeWorldCommit,
}

impl NativeAmxFaultPhase {
    const fn as_str(self) -> &'static str {
        match self {
            Self::AfterPrepareQc => "after_prepare_qc",
            Self::AfterCommitQc => "after_commit_qc",
            Self::BeforeWorldCommit => "before_world_commit",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "after_prepare_qc" => Ok(Self::AfterPrepareQc),
            "after_commit_qc" => Ok(Self::AfterCommitQc),
            "before_world_commit" => Ok(Self::BeforeWorldCommit),
            _ => Err(eyre!("unknown Native AMX fault phase `{value}`")),
        }
    }
}

/// Durable proof that the controlled daemon reached an exact Native AMX cut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeAmxFaultAck {
    /// Monotonic controller-local command revision.
    pub revision: u64,
    /// Exact phase reached before process abort.
    pub phase: NativeAmxFaultPhase,
    /// Exact Native AMX source transaction identity.
    pub source_id: [u8; 32],
}

impl ConsensusMessageControlAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Hold => "hold",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "drop" => Ok(Self::Drop),
            "hold" => Ok(Self::Hold),
            _ => Err(eyre!("unknown consensus message-control action `{value}`")),
        }
    }
}

/// One exact receiver-local inbound rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusMessageControlRule {
    /// Semantic sender identity.
    pub sender: PeerId,
    /// P2P identity which must authenticate the exact controlled copy.
    pub authenticated_via: PeerId,
    /// Exact v2 payload kind.
    pub kind: ConsensusMessageControlKind,
    /// Exact block height.
    pub height: u64,
    /// Exact consensus view.
    pub view: u64,
    /// Optional exact proposal block hash.
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Drop or bounded hold action.
    pub action: ConsensusMessageControlAction,
}

impl ConsensusMessageControlRule {
    /// Construct a hash-agnostic direct rule, authenticating via `sender`.
    pub fn exact(
        sender: PeerId,
        kind: ConsensusMessageControlKind,
        height: u64,
        view: u64,
        action: ConsensusMessageControlAction,
    ) -> Self {
        Self {
            authenticated_via: sender.clone(),
            sender,
            kind,
            height,
            view,
            block_hash: None,
            action,
        }
    }

    /// Construct a hash-agnostic rule for traffic forwarded by an explicit relay.
    pub fn relayed(
        sender: PeerId,
        authenticated_via: PeerId,
        kind: ConsensusMessageControlKind,
        height: u64,
        view: u64,
        action: ConsensusMessageControlAction,
    ) -> Self {
        Self {
            sender,
            authenticated_via,
            kind,
            height,
            view,
            block_hash: None,
            action,
        }
    }

    /// Further restrict this rule to one exact proposal block hash.
    #[must_use]
    pub fn with_block_hash(mut self, block_hash: HashOf<BlockHeader>) -> Self {
        self.block_hash = Some(block_hash);
        self
    }
}

/// Descriptor for one message retained by a receiver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusMessageControlHeld {
    /// Monotonic receiver-local ingress sequence.
    pub sequence: u64,
    /// Semantic origin carried by the P2P work item.
    pub sender: PeerId,
    /// P2P identity that authenticated the retained copy.
    pub authenticated_via: PeerId,
    /// Exact v2 payload kind.
    pub kind: ConsensusMessageControlKind,
    /// Message height, absent for compact chunk descriptors.
    pub height: Option<u64>,
    /// Message view, absent when the wire payload does not encode it directly.
    pub view: Option<u64>,
    /// Proposal block hash, when carried by the message.
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Complete block-and-payload subject, when carried by the message.
    pub subject: Option<BlockSubject>,
    /// Complete deterministic execution result, when carried by the message.
    pub execution_commitment: Option<ExecutionCommitment>,
    /// Inner validator signer or proposer index, when singular.
    pub signer: Option<ValidatorIndex>,
    /// Frozen-QC signer cited by a certified-body response.
    pub cited_responder: Option<ValidatorIndex>,
    /// Exact signer indices carried by a QC or TC envelope.
    pub certificate_signers: Vec<ValidatorIndex>,
    /// Digest of the canonical Sumeragi v2 envelope retained by this receiver.
    pub envelope_digest: CryptoHash,
    /// Original encoded P2P payload size.
    pub size_bytes: u64,
}

/// Canonical acknowledgement published by one controlled peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusMessageControlAck {
    /// Last atomically applied command revision.
    pub revision: u64,
    /// Digest of the exact canonical command bytes installed at `revision`.
    pub command_digest: CryptoHash,
    /// Exact active rule set acknowledged by the daemon.
    pub rules: Vec<ConsensusMessageControlRule>,
    /// Active count bound shared by held and in-flight releases.
    pub queue_capacity: usize,
    /// Currently retained messages.
    pub held: Vec<ConsensusMessageControlHeld>,
    /// Aggregate encoded bytes still resident in `held`.
    pub held_bytes: u64,
    /// Release sequences still awaiting a successful terminal outcome.
    pub release_pending: Vec<u64>,
    /// Release sequence currently crossing ordinary ingress.
    pub in_flight: Option<u64>,
    /// Exact encoded bytes owned by the in-flight release.
    pub in_flight_bytes: u64,
    /// Exact sequences delivered for the current revision.
    pub delivered: Vec<u64>,
    /// Exact sequences retired before ingress for the current revision.
    pub retired: Vec<u64>,
    /// Rule-matched messages intentionally dropped.
    pub dropped: u64,
    /// Rule-matched hold messages rejected because the queue was full.
    pub overflowed: u64,
    /// Commands rejected without changing the active revision.
    pub rejected_commands: u64,
    /// Stable error code for the last rejected/fatal operation.
    pub last_error: Option<String>,
    /// Whether the controller has permanently failed closed.
    pub fatal: bool,
    /// Whether a linearized FIFO drain is still retaining new v2 ingress.
    pub draining: bool,
    /// Revision that initiated the active or most recently completed drain.
    pub drain_fence: Option<u64>,
}

/// Per-peer handle for the feature-isolated controller.
#[derive(Debug)]
pub struct ConsensusMessageControl {
    root: PathBuf,
    root_identity: RootIdentity,
    initial_command: InitialCommand,
    next_revision: Mutex<u64>,
    next_native_amx_fault_revision: Mutex<u64>,
    operation: tokio::sync::Mutex<()>,
}

#[derive(Clone, Debug)]
struct InitialCommand {
    command_digest: CryptoHash,
    rules: Vec<ConsensusMessageControlRule>,
    queue_capacity: usize,
    staged: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RootIdentity {
    device: u64,
    inode: u64,
    owner: u32,
}

struct ExpectedAck<'a> {
    revision: u64,
    command_digest: CryptoHash,
    rules: &'a [ConsensusMessageControlRule],
    queue_capacity: usize,
    drain: bool,
}

impl ConsensusMessageControl {
    #[cfg(unix)]
    pub(crate) fn create(root: PathBuf) -> Result<Self> {
        fs::create_dir(&root).map_err(|error| {
            eyre!(
                "failed to create consensus message-control root {}: {error}",
                root.display()
            )
        })?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let root = root.canonicalize()?;
        let metadata = fs::symlink_metadata(&root)?;
        let root_identity = validate_private_root(&metadata)?;
        let mut control = Self {
            root,
            root_identity,
            initial_command: InitialCommand {
                command_digest: CryptoHash::new(b""),
                rules: Vec::new(),
                queue_capacity: DEFAULT_QUEUE_CAPACITY,
                staged: false,
            },
            next_revision: Mutex::new(1),
            next_native_amx_fault_revision: Mutex::new(0),
            operation: tokio::sync::Mutex::new(()),
        };
        let command_digest = control.write_command(1, &[], &[], DEFAULT_QUEUE_CAPACITY, false)?;
        control.initial_command.command_digest = command_digest;
        Ok(control)
    }

    #[cfg(not(unix))]
    pub(crate) fn create(_root: PathBuf) -> Result<Self> {
        Err(eyre!(
            "consensus message control requires Unix ownership/no-follow semantics"
        ))
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Replace the initial command before the daemon starts.
    ///
    /// The staged rules remain revision 1, so daemon initialization applies and
    /// acknowledges them before authenticated consensus ingress can run. This
    /// fails closed once an acknowledgement exists or another initial command
    /// has already been staged.
    pub(crate) fn stage_initial_rules(
        &mut self,
        rules: &[ConsensusMessageControlRule],
        queue_capacity: usize,
    ) -> Result<()> {
        let next_revision = self
            .next_revision
            .get_mut()
            .expect("message-control revision lock poisoned");
        if *next_revision != 1 {
            return Err(eyre!(
                "cannot stage initial consensus message-control rules after an update"
            ));
        }
        match fs::symlink_metadata(self.root.join(ACK_FILE)) {
            Ok(_) => {
                return Err(eyre!(
                    "cannot stage initial consensus message-control rules after daemon startup"
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        if self.initial_command.staged {
            return Err(eyre!(
                "initial consensus message-control rules were already staged"
            ));
        }
        let command_digest = self.write_command(1, rules, &[], queue_capacity, false)?;
        self.initial_command = InitialCommand {
            command_digest,
            rules: rules.to_vec(),
            queue_capacity,
            staged: true,
        };
        Ok(())
    }

    /// Wait until the daemon has pinned the directory and applied its initial command.
    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<ConsensusMessageControlAck> {
        self.wait_for_revision(
            ExpectedAck {
                revision: 1,
                command_digest: self.initial_command.command_digest,
                rules: &self.initial_command.rules,
                queue_capacity: self.initial_command.queue_capacity,
                drain: false,
            },
            timeout,
            None,
        )
        .await
    }

    /// Atomically install rules and optionally release exact retained sequences.
    pub async fn apply(
        &self,
        rules: &[ConsensusMessageControlRule],
        release: &[u64],
        queue_capacity: usize,
        timeout: Duration,
    ) -> Result<ConsensusMessageControlAck> {
        self.apply_command(rules, release, queue_capacity, false, timeout)
            .await
    }

    async fn apply_command(
        &self,
        rules: &[ConsensusMessageControlRule],
        release: &[u64],
        queue_capacity: usize,
        drain: bool,
        timeout: Duration,
    ) -> Result<ConsensusMessageControlAck> {
        let _operation = self.operation.lock().await;
        let rejected_before = self
            .read_ack()
            .map(|ack| ack.rejected_commands)
            .unwrap_or(0);
        let revision = {
            let mut next = self
                .next_revision
                .lock()
                .expect("message-control revision lock poisoned");
            *next = next
                .checked_add(1)
                .ok_or_else(|| eyre!("revision overflow"))?;
            *next
        };
        let command_digest = self.write_command(revision, rules, release, queue_capacity, drain)?;
        self.wait_for_revision(
            ExpectedAck {
                revision,
                command_digest,
                rules,
                queue_capacity,
                drain,
            },
            timeout,
            Some(rejected_before),
        )
        .await
    }

    /// Clear all rules and release every retained message, including messages
    /// that raced with the first acknowledgement.
    pub async fn heal_and_release_all(
        &self,
        timeout: Duration,
    ) -> Result<ConsensusMessageControlAck> {
        let ack = self
            .apply_command(&[], &[], DEFAULT_QUEUE_CAPACITY, true, timeout)
            .await?;
        if ack.draining
            || ack.drain_fence != Some(ack.revision)
            || !ack.held.is_empty()
            || !ack.release_pending.is_empty()
            || ack.in_flight.is_some()
        {
            return Err(eyre!(
                "consensus message-control drain fence did not complete atomically"
            ));
        }
        Ok(ack)
    }

    /// Read and validate the latest stable canonical acknowledgement.
    pub fn read_ack(&self) -> Result<ConsensusMessageControlAck> {
        validate_root_identity(&self.root, self.root_identity)?;
        let bytes = read_bounded_private_file(
            &self.root.join(ACK_FILE),
            MAX_ACK_BYTES,
            self.root_identity.owner,
        )?;
        validate_root_identity(&self.root, self.root_identity)?;
        parse_ack(&bytes)
    }

    /// Arm one exact, one-shot Native AMX process cut for this peer.
    ///
    /// `source_id` is the 32-byte digest of the exact signed source transaction,
    /// not its external-entrypoint projection.
    ///
    /// The feature-isolated daemon fsyncs an acknowledgement at the named
    /// phase and then aborts. Restart sees the acknowledgement and will not
    /// repeat the same revision.
    pub fn arm_native_amx_fault(
        &self,
        phase: NativeAmxFaultPhase,
        source_id: [u8; 32],
    ) -> Result<u64> {
        validate_root_identity(&self.root, self.root_identity)?;
        let mut next = self
            .next_native_amx_fault_revision
            .lock()
            .expect("Native AMX fault revision lock poisoned");
        *next = next
            .checked_add(1)
            .ok_or_else(|| eyre!("Native AMX fault revision overflow"))?;
        let revision = *next;
        let command = native_amx_fault_value(revision, phase, source_id);
        let bytes = canonical_json(&command)?;
        write_atomic_private_file(
            &self.root,
            NATIVE_AMX_FAULT_COMMAND_FILE,
            &bytes,
            self.root_identity.owner,
        )?;
        validate_root_identity(&self.root, self.root_identity)?;
        drop(next);
        Ok(revision)
    }

    /// Read and authenticate the latest durable Native AMX phase acknowledgement.
    pub fn read_native_amx_fault_ack(&self) -> Result<NativeAmxFaultAck> {
        validate_root_identity(&self.root, self.root_identity)?;
        let bytes = read_bounded_private_file(
            &self.root.join(NATIVE_AMX_FAULT_ACK_FILE),
            MAX_CONTROL_BYTES,
            self.root_identity.owner,
        )?;
        validate_root_identity(&self.root, self.root_identity)?;
        parse_native_amx_fault(&bytes)
    }

    /// Wait until the daemon durably proves that it reached the armed phase.
    pub async fn wait_for_native_amx_fault(
        &self,
        revision: u64,
        phase: NativeAmxFaultPhase,
        source_id: [u8; 32],
        timeout: Duration,
    ) -> Result<NativeAmxFaultAck> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.read_native_amx_fault_ack() {
                Ok(ack)
                    if ack.revision == revision
                        && ack.phase == phase
                        && ack.source_id == source_id =>
                {
                    return Ok(ack);
                }
                Ok(ack) if ack.revision >= revision => {
                    return Err(eyre!(
                        "Native AMX fault acknowledgement differs from revision {revision}: {ack:?}"
                    ));
                }
                Ok(_) | Err(_) if Instant::now() < deadline => {}
                Err(error) => return Err(error),
                Ok(ack) => {
                    return Err(eyre!(
                        "timed out waiting for Native AMX fault revision {revision}; latest={ack:?}"
                    ));
                }
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for Native AMX fault revision {revision}"
                ));
            }
            sleep(ACK_POLL).await;
        }
    }

    async fn wait_for_revision(
        &self,
        expected: ExpectedAck<'_>,
        timeout: Duration,
        rejected_before: Option<u64>,
    ) -> Result<ConsensusMessageControlAck> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.read_ack() {
                Ok(ack) if ack.fatal => {
                    return Err(eyre!(
                        "consensus message controller failed closed: {:?}",
                        ack.last_error
                    ));
                }
                Ok(ack)
                    if ack_matches_expected(&ack, &expected)
                        && ack.release_pending.is_empty()
                        && ack.in_flight.is_none() =>
                {
                    return Ok(ack);
                }
                Ok(ack) if ack_matches_expected_release_in_progress(&ack, &expected) => {
                    // The daemon has installed the exact non-drain command,
                    // but one or more explicitly released occurrences still
                    // own the controller or ordinary ingress. Keep waiting for
                    // their terminal delivered/retired acknowledgement instead
                    // of misclassifying the matching revision as command drift.
                }
                Ok(ack)
                    if rejected_before.is_some_and(|baseline| ack.rejected_commands > baseline)
                        && ack.revision < expected.revision
                        && ack.last_error.is_some() =>
                {
                    return Err(eyre!(
                        "consensus message-control revision {revision} was rejected: {error:?}",
                        revision = expected.revision,
                        error = ack.last_error,
                    ));
                }
                Ok(ack)
                    if ack.revision == expected.revision
                        && expected.drain
                        && ack.command_digest == expected.command_digest
                        && ack.queue_capacity == expected.queue_capacity
                        && ack.draining
                        && ack.drain_fence == Some(expected.revision) =>
                {
                    // The exact command is active, but the FIFO fence has not
                    // yet drained all pre-cutover and racing v2 ingress.
                }
                Ok(ack) if ack.revision >= expected.revision => {
                    return Err(eyre!(
                        "message-control acknowledgement does not bind requested revision {}: acknowledged revision={}, expected_digest={}, acknowledged_digest={}, digest_match={}, rules_match={}, expected_queue_capacity={}, acknowledged_queue_capacity={}, draining={}, drain_fence={:?}, release_pending={}, in_flight={:?}",
                        expected.revision,
                        ack.revision,
                        expected.command_digest,
                        ack.command_digest,
                        ack.command_digest == expected.command_digest,
                        ack.rules == expected.rules,
                        expected.queue_capacity,
                        ack.queue_capacity,
                        ack.draining,
                        ack.drain_fence,
                        ack.release_pending.len(),
                        ack.in_flight,
                    ));
                }
                Ok(_) => {}
                Err(error) if Instant::now() < deadline => {
                    let _ = error;
                }
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "timed out waiting for consensus message-control revision {revision}",
                    revision = expected.revision
                ));
            }
            sleep(ACK_POLL).await;
        }
    }

    fn write_command(
        &self,
        revision: u64,
        rules: &[ConsensusMessageControlRule],
        release: &[u64],
        queue_capacity: usize,
        drain: bool,
    ) -> Result<CryptoHash> {
        if rules.len() > MAX_RULES {
            return Err(eyre!("too many message-control rules"));
        }
        if rules
            .iter()
            .any(|rule| rule.height == 0 || !rule.kind.has_exact_round())
        {
            return Err(eyre!(
                "message-control rules require a positive, directly encoded round"
            ));
        }
        for (index, rule) in rules.iter().enumerate() {
            if rules[..index].iter().any(|prior| {
                prior.sender == rule.sender
                    && prior.authenticated_via == rule.authenticated_via
                    && prior.kind == rule.kind
                    && prior.height == rule.height
                    && prior.view == rule.view
                    && (prior.block_hash.is_none()
                        || rule.block_hash.is_none()
                        || prior.block_hash == rule.block_hash)
            }) {
                return Err(eyre!("ambiguous overlapping message-control rules"));
            }
        }
        if release.len() > MAX_RELEASES {
            return Err(eyre!("too many message-control release entries"));
        }
        if queue_capacity == 0 || queue_capacity > MAX_HOLDS {
            return Err(eyre!("invalid message-control queue capacity"));
        }
        if release.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(eyre!("release sequences must be strictly increasing"));
        }
        if drain && !release.is_empty() {
            return Err(eyre!("drain commands cannot carry explicit releases"));
        }
        let rules = rules.iter().map(rule_value).collect::<Vec<_>>();
        let release = release.iter().copied().map(Value::from).collect::<Vec<_>>();
        let value = object_value([
            ("drain", Value::from(drain)),
            (
                "queue_capacity",
                Value::from(u64::try_from(queue_capacity)?),
            ),
            ("release", Value::Array(release)),
            ("revision", Value::from(revision)),
            ("rules", Value::Array(rules)),
            ("version", Value::from(FORMAT_VERSION)),
        ]);
        let bytes = canonical_json(&value)?;
        if bytes.len() > MAX_CONTROL_BYTES {
            return Err(eyre!("encoded message-control command is too large"));
        }
        validate_root_identity(&self.root, self.root_identity)?;
        write_atomic_private_file(&self.root, CONTROL_FILE, &bytes, self.root_identity.owner)?;
        validate_root_identity(&self.root, self.root_identity)?;
        Ok(CryptoHash::new(&bytes))
    }
}

fn ack_matches_expected(ack: &ConsensusMessageControlAck, expected: &ExpectedAck<'_>) -> bool {
    ack.revision == expected.revision
        && ack.command_digest == expected.command_digest
        && ack.rules == expected.rules
        && ack.queue_capacity == expected.queue_capacity
        && !ack.draining
        && (!expected.drain || ack.drain_fence == Some(expected.revision))
}

fn ack_matches_expected_release_in_progress(
    ack: &ConsensusMessageControlAck,
    expected: &ExpectedAck<'_>,
) -> bool {
    !expected.drain
        && ack_matches_expected(ack, expected)
        && (!ack.release_pending.is_empty() || ack.in_flight.is_some())
}

fn rule_value(rule: &ConsensusMessageControlRule) -> Value {
    object_value([
        ("action", Value::from(rule.action.as_str())),
        (
            "authenticated_via",
            Value::from(rule.authenticated_via.to_string()),
        ),
        (
            "block_hash",
            rule.block_hash
                .as_ref()
                .map_or(Value::Null, |value| Value::from(value.to_string())),
        ),
        ("height", Value::from(rule.height)),
        ("kind", Value::from(rule.kind.as_str())),
        ("sender", Value::from(rule.sender.to_string())),
        ("view", Value::from(rule.view)),
    ])
}

fn native_amx_fault_value(revision: u64, phase: NativeAmxFaultPhase, source_id: [u8; 32]) -> Value {
    object_value([
        ("phase", Value::from(phase.as_str())),
        ("revision", Value::from(revision)),
        ("source_id", Value::from(lowercase_hex(&source_id))),
        ("version", Value::from(NATIVE_AMX_FAULT_FORMAT_VERSION)),
    ])
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_canonical_lower_hex_32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!(
            "Native AMX fault acknowledgement source is not canonical"
        ));
    }

    let mut decoded = [0_u8; 32];
    for (output, pair) in decoded.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let nibble = |byte: u8| match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => unreachable!("canonical lowercase hexadecimal was validated"),
        };
        *output = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    Ok(decoded)
}

fn parse_native_amx_fault(bytes: &[u8]) -> Result<NativeAmxFaultAck> {
    if bytes.is_empty() || bytes.len() > MAX_CONTROL_BYTES {
        return Err(eyre!("Native AMX fault acknowledgement has invalid size"));
    }
    let value: Value = norito::json::from_slice(bytes)?;
    if canonical_json(&value)?.as_slice() != bytes {
        return Err(eyre!("Native AMX fault acknowledgement is not canonical"));
    }
    let object = exact_object(
        &value,
        &["phase", "revision", "source_id", "version"],
        "Native AMX fault acknowledgement",
    )?;
    if required_u64(object, "version")? != NATIVE_AMX_FAULT_FORMAT_VERSION {
        return Err(eyre!(
            "unsupported Native AMX fault acknowledgement version"
        ));
    }
    let revision = required_u64(object, "revision")?;
    if revision == 0 {
        return Err(eyre!(
            "Native AMX fault acknowledgement revision must be positive"
        ));
    }
    let phase = object
        .get("phase")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("Native AMX fault acknowledgement lacks `phase`"))?;
    let phase = NativeAmxFaultPhase::parse(phase)?;
    let source = object
        .get("source_id")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("Native AMX fault acknowledgement lacks `source_id`"))?;
    let source_id = decode_canonical_lower_hex_32(source)?;
    Ok(NativeAmxFaultAck {
        revision,
        phase,
        source_id,
    })
}

fn parse_ack(bytes: &[u8]) -> Result<ConsensusMessageControlAck> {
    if bytes.len() > MAX_ACK_BYTES {
        return Err(eyre!("message-control acknowledgement is too large"));
    }
    let value: Value = norito::json::from_slice(bytes)?;
    if canonical_json(&value)?.as_slice() != bytes {
        return Err(eyre!("message-control acknowledgement is not canonical"));
    }
    let object = exact_object(
        &value,
        &[
            "command_digest",
            "delivered",
            "dropped",
            "drain_fence",
            "draining",
            "fatal",
            "held",
            "held_bytes",
            "in_flight",
            "in_flight_bytes",
            "last_error",
            "overflowed",
            "queue_capacity",
            "rejected_commands",
            "release_pending",
            "retired",
            "revision",
            "rules",
            "version",
        ],
        "acknowledgement",
    )?;
    if required_u64(object, "version")? != FORMAT_VERSION {
        return Err(eyre!("unsupported message-control acknowledgement version"));
    }
    let held_values = object
        .get("held")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("message-control acknowledgement lacks `held`"))?;
    if held_values.len() > MAX_HOLDS {
        return Err(eyre!(
            "message-control acknowledgement has too many held entries"
        ));
    }
    let held = held_values
        .iter()
        .map(parse_held)
        .collect::<Result<Vec<_>>>()?;
    if held
        .windows(2)
        .any(|pair| pair[0].sequence >= pair[1].sequence)
    {
        return Err(eyre!("held message sequences are not strictly increasing"));
    }
    let held_bytes = held.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.size_bytes)
            .ok_or_else(|| eyre!("held-byte acknowledgement overflow"))
    })?;
    if held_bytes > MAX_HELD_BYTES || held_bytes != required_u64(object, "held_bytes")? {
        return Err(eyre!("held-byte acknowledgement mismatch"));
    }
    let release_pending = parse_u64_array(object, "release_pending", MAX_RELEASES)?;
    let delivered = parse_u64_array(object, "delivered", MAX_RELEASES)?;
    let retired = parse_u64_array(object, "retired", MAX_RELEASES)?;
    require_strictly_increasing_positive(&release_pending, "release_pending")?;
    require_strictly_increasing_positive(&delivered, "delivered")?;
    require_strictly_increasing_positive(&retired, "retired")?;
    let in_flight = match object.get("in_flight") {
        Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|sequence| *sequence > 0)
                .ok_or_else(|| eyre!("invalid message-control in-flight sequence"))?,
        ),
        None => return Err(eyre!("message-control acknowledgement lacks `in_flight`")),
    };
    let in_flight_bytes = required_u64(object, "in_flight_bytes")?;
    if in_flight.is_some() != (in_flight_bytes > 0) {
        return Err(eyre!(
            "message-control in-flight sequence and byte ownership disagree"
        ));
    }
    let held_sequences = held
        .iter()
        .map(|entry| entry.sequence)
        .collect::<BTreeSet<_>>();
    if release_pending
        .iter()
        .any(|sequence| !held_sequences.contains(sequence))
    {
        return Err(eyre!("pending release is not present in the held queue"));
    }
    if delivered
        .iter()
        .any(|sequence| held_sequences.contains(sequence))
        || retired.iter().any(|sequence| {
            held_sequences.contains(sequence)
                || release_pending.contains(sequence)
                || delivered.contains(sequence)
        })
        || in_flight.is_some_and(|sequence| {
            held_sequences.contains(&sequence)
                || release_pending.contains(&sequence)
                || delivered.contains(&sequence)
                || retired.contains(&sequence)
        })
    {
        return Err(eyre!("message-control sequence sets are inconsistent"));
    }
    let draining = object
        .get("draining")
        .and_then(Value::as_bool)
        .ok_or_else(|| eyre!("message-control acknowledgement lacks `draining`"))?;
    let drain_fence = match object.get("drain_fence") {
        Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|revision| *revision > 0)
                .ok_or_else(|| eyre!("invalid message-control drain fence"))?,
        ),
        None => return Err(eyre!("message-control acknowledgement lacks `drain_fence`")),
    };
    if draining && held_sequences != release_pending.iter().copied().collect()
        || draining && drain_fence.is_none()
    {
        return Err(eyre!(
            "active drain acknowledgement is internally inconsistent"
        ));
    }
    let queue_capacity = required_u64(object, "queue_capacity")?;
    if queue_capacity == 0
        || queue_capacity > u64::try_from(MAX_HOLDS)?
        || queue_capacity
            < u64::try_from(held.len())?
                .checked_add(if in_flight.is_some() { 1 } else { 0 })
                .ok_or_else(|| eyre!("acknowledged hold count overflow"))?
    {
        return Err(eyre!("invalid acknowledged hold queue capacity"));
    }
    if held_bytes
        .checked_add(in_flight_bytes)
        .is_none_or(|bytes| bytes > MAX_HELD_BYTES)
    {
        return Err(eyre!(
            "acknowledged retained bytes exceed the controller bound"
        ));
    }
    let rules = parse_ack_rules(object)?;
    let command_digest = match object.get("command_digest") {
        Some(Value::Null) => return Err(eyre!("applied acknowledgement lacks command digest")),
        Some(value) => {
            let literal = value
                .as_str()
                .ok_or_else(|| eyre!("invalid command digest"))?;
            let parsed = literal.parse::<iroha_crypto::Hash>()?;
            if parsed.to_string() != literal {
                return Err(eyre!("noncanonical command digest"));
            }
            parsed
        }
        None => return Err(eyre!("acknowledgement lacks command digest")),
    };
    let overflowed = required_u64(object, "overflowed")?;
    let fatal = object
        .get("fatal")
        .and_then(Value::as_bool)
        .ok_or_else(|| eyre!("message-control acknowledgement lacks `fatal`"))?;
    if overflowed > 0 && !fatal {
        return Err(eyre!("hold overflow must fail the controller closed"));
    }
    let revision = required_u64(object, "revision")?;
    if revision == 0 {
        return Err(eyre!("acknowledged revision must be positive"));
    }
    if drain_fence.is_some_and(|fence| fence > revision) {
        return Err(eyre!("drain fence exceeds acknowledged revision"));
    }
    let ack = ConsensusMessageControlAck {
        revision,
        command_digest,
        rules,
        queue_capacity: usize::try_from(queue_capacity)?,
        held,
        held_bytes,
        release_pending,
        in_flight,
        in_flight_bytes,
        delivered,
        retired,
        dropped: required_u64(object, "dropped")?,
        overflowed,
        rejected_commands: required_u64(object, "rejected_commands")?,
        last_error: optional_string(object, "last_error")?,
        fatal,
        draining,
        drain_fence,
    };
    if ack.fatal && ack.last_error.is_none() {
        return Err(eyre!(
            "fatal controller acknowledgement needs an error code"
        ));
    }
    Ok(ack)
}

fn parse_held(value: &Value) -> Result<ConsensusMessageControlHeld> {
    let object = exact_object(
        value,
        &[
            "authenticated_via",
            "block_hash",
            "certificate_signers",
            "cited_responder",
            "envelope_digest",
            "execution_commitment",
            "height",
            "kind",
            "sender",
            "sequence",
            "signer",
            "size_bytes",
            "subject",
            "view",
        ],
        "held descriptor",
    )?;
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("held message descriptor lacks `kind`"))?;
    let sequence = required_u64(object, "sequence")?;
    let height = optional_u64(object, "height")?;
    let view = optional_u64(object, "view")?;
    let size_bytes = required_u64(object, "size_bytes")?;
    if sequence == 0 || height == Some(0) || size_bytes == 0 {
        return Err(eyre!("held descriptor has a zero required integer"));
    }
    let kind = ConsensusMessageControlKind::parse(kind)?;
    if kind == ConsensusMessageControlKind::PayloadChunk {
        if height.is_some() || view.is_some() {
            return Err(eyre!("payload chunk descriptor cannot invent a round"));
        }
    } else if kind == ConsensusMessageControlKind::CommitCertificateRequest {
        if height.is_none() || view.is_some() {
            return Err(eyre!(
                "commit-certificate request descriptor has an invalid round"
            ));
        }
    } else if height.is_none() || view.is_none() {
        return Err(eyre!("round-carrying held descriptor lacks its round"));
    }
    let sender = parse_canonical_peer(object, "sender")?;
    let authenticated_via = parse_canonical_peer(object, "authenticated_via")?;
    let subject = match object.get("subject") {
        Some(Value::Null) => None,
        Some(value) => Some(
            norito::json::from_value::<BlockSubject>(value.clone())
                .map_err(|error| eyre!("invalid held-message subject: {error}"))?,
        ),
        None => return Err(eyre!("held descriptor lacks `subject`")),
    };
    let execution_commitment = match object.get("execution_commitment") {
        Some(Value::Null) => None,
        Some(value) => Some(
            norito::json::from_value::<ExecutionCommitment>(value.clone())
                .map_err(|error| eyre!("invalid held-message execution commitment: {error}"))?,
        ),
        None => return Err(eyre!("held descriptor lacks `execution_commitment`")),
    };
    if let Some(commitment) = &execution_commitment {
        commitment
            .validate()
            .map_err(|error| eyre!("invalid held-message execution commitment: {error}"))?;
    }
    let block_hash = parse_optional_canonical_hash(object, "block_hash")?;
    if block_hash != subject.map(|subject| subject.block_hash)
        || execution_commitment.is_some() && subject.is_none()
    {
        return Err(eyre!(
            "held descriptor has inconsistent subject, block hash, or execution commitment"
        ));
    }
    let signer = optional_u64(object, "signer")?
        .map(ValidatorIndex::try_from)
        .transpose()
        .map_err(|_| eyre!("held descriptor signer exceeds the validator-index range"))?;
    let cited_responder = optional_u64(object, "cited_responder")?
        .map(ValidatorIndex::try_from)
        .transpose()
        .map_err(|_| eyre!("held descriptor cited responder exceeds the validator-index range"))?;
    let requires_single_signer = matches!(
        kind,
        ConsensusMessageControlKind::Proposal
            | ConsensusMessageControlKind::PrepareVote
            | ConsensusMessageControlKind::CommitVote
            | ConsensusMessageControlKind::TimeoutVote
            | ConsensusMessageControlKind::PayloadChunk
    );
    if requires_single_signer != signer.is_some() {
        return Err(eyre!(
            "held descriptor disagrees with its payload kind about the inner signer"
        ));
    }
    if (kind == ConsensusMessageControlKind::CertifiedBodyResponse) != cited_responder.is_some() {
        return Err(eyre!(
            "held descriptor disagrees with its payload kind about the cited responder"
        ));
    }
    let certificate_signers =
        parse_u64_array(object, "certificate_signers", MAX_VALIDATORS_PER_HEIGHT)?
            .into_iter()
            .map(|signer| {
                ValidatorIndex::try_from(signer)
                    .map_err(|_| eyre!("certificate signer exceeds the validator-index range"))
            })
            .collect::<Result<Vec<_>>>()?;
    if certificate_signers
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(eyre!(
            "held descriptor certificate signers are not strictly increasing"
        ));
    }
    let has_subject_and_execution = subject.is_some() && execution_commitment.is_some();
    let has_no_subject_or_execution = subject.is_none() && execution_commitment.is_none();
    let has_single_signer = signer.is_some();
    let has_certificate_signers = !certificate_signers.is_empty();
    let valid_payload_shape = match kind {
        ConsensusMessageControlKind::Proposal => {
            subject.is_some()
                && execution_commitment.is_none()
                && has_single_signer
                && !has_certificate_signers
        }
        ConsensusMessageControlKind::PrepareVote | ConsensusMessageControlKind::CommitVote => {
            has_subject_and_execution && has_single_signer && !has_certificate_signers
        }
        ConsensusMessageControlKind::PrepareCertificate
        | ConsensusMessageControlKind::CommitCertificate
        | ConsensusMessageControlKind::CertifiedBodyRequest
        | ConsensusMessageControlKind::CommitCertificateResponse => {
            has_subject_and_execution && !has_single_signer && has_certificate_signers
        }
        ConsensusMessageControlKind::TimeoutVote => {
            (has_subject_and_execution || has_no_subject_or_execution)
                && has_single_signer
                && !has_certificate_signers
        }
        ConsensusMessageControlKind::TimeoutCertificate => {
            (has_subject_and_execution || has_no_subject_or_execution)
                && !has_single_signer
                && has_certificate_signers
        }
        ConsensusMessageControlKind::PayloadManifest => {
            subject.is_some()
                && execution_commitment.is_none()
                && !has_single_signer
                && !has_certificate_signers
        }
        ConsensusMessageControlKind::PayloadChunk => {
            has_no_subject_or_execution && has_single_signer && !has_certificate_signers
        }
        ConsensusMessageControlKind::CertifiedBodyResponse => {
            subject.is_some()
                && execution_commitment.is_none()
                && !has_single_signer
                && !has_certificate_signers
        }
        ConsensusMessageControlKind::CommitCertificateRequest => {
            has_no_subject_or_execution && !has_single_signer && !has_certificate_signers
        }
    };
    if !valid_payload_shape {
        return Err(eyre!(
            "held descriptor fields disagree with the exact payload-kind shape"
        ));
    }
    Ok(ConsensusMessageControlHeld {
        sequence,
        sender,
        authenticated_via,
        kind,
        height,
        view,
        block_hash,
        subject,
        execution_commitment,
        signer,
        cited_responder,
        certificate_signers,
        envelope_digest: parse_canonical_crypto_hash(object, "envelope_digest")?,
        size_bytes,
    })
}

fn parse_ack_rules(object: &Map) -> Result<Vec<ConsensusMessageControlRule>> {
    let values = object
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("acknowledgement lacks rules"))?;
    if values.len() > MAX_RULES {
        return Err(eyre!("acknowledgement has too many rules"));
    }
    let mut rules = Vec::with_capacity(values.len());
    for value in values {
        let object = exact_object(
            value,
            &[
                "action",
                "authenticated_via",
                "block_hash",
                "height",
                "kind",
                "sender",
                "view",
            ],
            "acknowledged rule",
        )?;
        let height = required_u64(object, "height")?;
        if height == 0 {
            return Err(eyre!("acknowledged rule height must be positive"));
        }
        let kind = ConsensusMessageControlKind::parse(
            object
                .get("kind")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("acknowledged rule lacks kind"))?,
        )?;
        if !kind.has_exact_round() {
            return Err(eyre!("acknowledged rule kind has no exact wire round"));
        }
        let action = ConsensusMessageControlAction::parse(
            object
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| eyre!("acknowledged rule lacks action"))?,
        )?;
        let rule = ConsensusMessageControlRule {
            sender: parse_canonical_peer(object, "sender")?,
            authenticated_via: parse_canonical_peer(object, "authenticated_via")?,
            kind,
            height,
            view: required_u64(object, "view")?,
            block_hash: parse_optional_canonical_hash(object, "block_hash")?,
            action,
        };
        if rules.iter().any(|prior: &ConsensusMessageControlRule| {
            prior.sender == rule.sender
                && prior.authenticated_via == rule.authenticated_via
                && prior.kind == rule.kind
                && prior.height == rule.height
                && prior.view == rule.view
                && (prior.block_hash.is_none()
                    || rule.block_hash.is_none()
                    || prior.block_hash == rule.block_hash)
        }) {
            return Err(eyre!("acknowledgement contains ambiguous rules"));
        }
        rules.push(rule);
    }
    Ok(rules)
}

fn parse_u64_array(object: &Map, field: &str, max: usize) -> Result<Vec<u64>> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("message-control acknowledgement lacks `{field}`"))?;
    if values.len() > max {
        return Err(eyre!(
            "message-control acknowledgement `{field}` is too large"
        ));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| eyre!("invalid `{field}` entry"))
        })
        .collect()
}

fn require_strictly_increasing_positive(values: &[u64], field: &str) -> Result<()> {
    if values.first().is_some_and(|value| *value == 0)
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(eyre!("`{field}` must be strictly increasing and positive"));
    }
    Ok(())
}

fn exact_object<'a>(value: &'a Value, fields: &[&str], label: &str) -> Result<&'a Map> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("message-control {label} is not an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(eyre!("message-control {label} has unexpected fields"));
    }
    Ok(object)
}

fn parse_canonical_peer(object: &Map, field: &str) -> Result<PeerId> {
    let literal = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("message-control record lacks peer `{field}`"))?;
    let parsed = literal.parse::<PeerId>()?;
    if parsed.to_string() != literal {
        return Err(eyre!("message-control peer `{field}` is not canonical"));
    }
    Ok(parsed)
}

fn parse_optional_canonical_hash(object: &Map, field: &str) -> Result<Option<HashOf<BlockHeader>>> {
    let Some(value) = object.get(field) else {
        return Err(eyre!("message-control record lacks hash `{field}`"));
    };
    let Some(literal) = value.as_str() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(eyre!("message-control hash `{field}` is invalid"));
    };
    let parsed = literal.parse::<HashOf<BlockHeader>>()?;
    if parsed.to_string() != literal {
        return Err(eyre!("message-control hash `{field}` is not canonical"));
    }
    Ok(Some(parsed))
}

fn parse_canonical_crypto_hash(object: &Map, field: &str) -> Result<CryptoHash> {
    let literal = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("message-control record lacks hash `{field}`"))?;
    let parsed = literal.parse::<CryptoHash>()?;
    if parsed.to_string() != literal {
        return Err(eyre!("message-control hash `{field}` is not canonical"));
    }
    Ok(parsed)
}

fn required_u64(object: &Map, field: &str) -> Result<u64> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("message-control acknowledgement lacks integer `{field}`"))
}

fn optional_u64(object: &Map, field: &str) -> Result<Option<u64>> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| eyre!("message-control acknowledgement has invalid `{field}`")),
        None => Err(eyre!(
            "message-control acknowledgement lacks optional integer `{field}`"
        )),
    }
}

fn optional_string(object: &Map, field: &str) -> Result<Option<String>> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| eyre!("message-control acknowledgement has invalid `{field}`")),
        None => Err(eyre!("message-control acknowledgement lacks `{field}`")),
    }
}

fn canonical_json(value: &Value) -> Result<Vec<u8>> {
    Ok(norito::json::to_json(value)?.into_bytes())
}

fn object_value<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in entries {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}

#[cfg(unix)]
fn read_bounded_private_file(path: &Path, max_bytes: usize, owner: u32) -> Result<Vec<u8>> {
    use std::os::unix::fs::OpenOptionsExt;

    let named_before = fs::symlink_metadata(path)?;
    validate_private_file(&named_before, owner)?;
    if usize::try_from(named_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(eyre!("message-control acknowledgement is too large"));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits())
        .open(path)?;
    let opened_before = file.metadata()?;
    validate_private_file(&opened_before, owner)?;
    if !same_file(&named_before, &opened_before) {
        return Err(eyre!("message-control acknowledgement identity changed"));
    }
    if usize::try_from(opened_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(eyre!("message-control acknowledgement is too large"));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_before.len())
            .unwrap_or(max_bytes)
            .min(max_bytes),
    );
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes)? + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(eyre!("message-control acknowledgement is too large"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut confirmation = Vec::with_capacity(bytes.len());
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes)? + 1)
        .read_to_end(&mut confirmation)?;
    if confirmation != bytes {
        return Err(eyre!(
            "message-control acknowledgement changed while confirming"
        ));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    validate_private_file(&opened_after, owner)?;
    validate_private_file(&named_after, owner)?;
    if !same_file(&opened_before, &opened_after)
        || !same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != u64::try_from(bytes.len())?
        || opened_before.modified().ok() != opened_after.modified().ok()
    {
        return Err(eyre!(
            "message-control acknowledgement changed while reading"
        ));
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_bounded_private_file(_path: &Path, _max_bytes: usize, _owner: u32) -> Result<Vec<u8>> {
    Err(eyre!(
        "consensus message control requires Unix ownership/no-follow semantics"
    ))
}

fn write_atomic_private_file(root: &Path, name: &str, bytes: &[u8], owner: u32) -> Result<()> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp = root.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
    let final_path = root.join(name);
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => validate_private_file(&metadata, owner)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        let written = file.metadata()?;
        validate_private_file(&written, owner)?;
        if written.len() != u64::try_from(bytes.len())? {
            return Err(eyre!("message-control write length changed before install"));
        }
        fs::rename(&temp, &final_path)?;
        let installed = fs::symlink_metadata(&final_path)?;
        validate_private_file(&installed, owner)?;
        if !same_file(&written, &installed) || installed.len() != written.len() {
            return Err(eyre!(
                "message-control file identity changed during atomic install"
            ));
        }
        File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(unix)]
fn validate_private_root(metadata: &fs::Metadata) -> Result<RootIdentity> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.file_type().is_dir()
        || metadata.file_type().is_symlink()
        || metadata.mode() & 0o777 != 0o700
    {
        return Err(eyre!("unsafe consensus message-control root"));
    }
    Ok(RootIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        owner: metadata.uid(),
    })
}

#[cfg(not(unix))]
fn validate_private_root(_metadata: &fs::Metadata) -> Result<RootIdentity> {
    Err(eyre!(
        "consensus message control requires Unix ownership/no-follow semantics"
    ))
}

fn validate_root_identity(root: &Path, expected: RootIdentity) -> Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    let actual = validate_private_root(&metadata)?;
    if actual != expected {
        return Err(eyre!("consensus message-control root identity changed"));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_private_file(metadata: &fs::Metadata, owner: u32) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.uid() != owner
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!("unsafe consensus message-control file"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_private_file(_metadata: &fs::Metadata, _owner: u32) -> Result<()> {
    Err(eyre!(
        "consensus message control requires Unix ownership/no-follow semantics"
    ))
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use tempfile::tempdir;

    fn descriptor_peer() -> PeerId {
        PeerId::new(
            KeyPair::try_from_seed(vec![0x33; 32], Algorithm::Ed25519)
                .expect("deterministic descriptor peer")
                .public_key()
                .clone(),
        )
    }

    fn descriptor_subject() -> BlockSubject {
        BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(CryptoHash::new(
                b"descriptor-parent",
            ))),
            block_hash: HashOf::from_untyped_unchecked(CryptoHash::new(b"descriptor-block")),
            payload_hash: CryptoHash::new(b"descriptor-payload"),
        }
    }

    fn descriptor_execution_commitment() -> ExecutionCommitment {
        ExecutionCommitment::without_topups_or_merge_carrier(
            CryptoHash::new(b"descriptor-parent-state"),
            CryptoHash::new(b"descriptor-post-state"),
            CryptoHash::new(b"descriptor-writes"),
            1,
            CryptoHash::new(b"descriptor-executed-wire"),
        )
    }

    fn held_descriptor(kind: ConsensusMessageControlKind) -> Value {
        let peer = descriptor_peer().to_string();
        let subject = descriptor_subject();
        let subject_value = norito::json::to_value(&subject).expect("encode descriptor subject");
        let commitment_value = norito::json::to_value(&descriptor_execution_commitment())
            .expect("encode descriptor execution commitment");
        let cited_responder = if kind == ConsensusMessageControlKind::CertifiedBodyResponse {
            Value::from(0_u64)
        } else {
            Value::Null
        };
        let (height, view, subject, execution, signer, certificate_signers) = match kind {
            ConsensusMessageControlKind::Proposal => (
                Value::from(9_u64),
                Value::from(2_u64),
                subject_value,
                Value::Null,
                Value::from(0_u64),
                Vec::new(),
            ),
            ConsensusMessageControlKind::PrepareVote | ConsensusMessageControlKind::CommitVote => (
                Value::from(9_u64),
                Value::from(2_u64),
                subject_value,
                commitment_value,
                Value::from(0_u64),
                Vec::new(),
            ),
            ConsensusMessageControlKind::PrepareCertificate
            | ConsensusMessageControlKind::CommitCertificate
            | ConsensusMessageControlKind::CertifiedBodyRequest
            | ConsensusMessageControlKind::CommitCertificateResponse => (
                Value::from(9_u64),
                Value::from(2_u64),
                subject_value,
                commitment_value,
                Value::Null,
                vec![Value::from(0_u64), Value::from(1_u64), Value::from(2_u64)],
            ),
            ConsensusMessageControlKind::TimeoutVote => (
                Value::from(9_u64),
                Value::from(2_u64),
                Value::Null,
                Value::Null,
                Value::from(0_u64),
                Vec::new(),
            ),
            ConsensusMessageControlKind::TimeoutCertificate => (
                Value::from(9_u64),
                Value::from(2_u64),
                Value::Null,
                Value::Null,
                Value::Null,
                vec![Value::from(0_u64), Value::from(1_u64), Value::from(2_u64)],
            ),
            ConsensusMessageControlKind::PayloadManifest => (
                Value::from(9_u64),
                Value::from(2_u64),
                subject_value,
                Value::Null,
                Value::Null,
                Vec::new(),
            ),
            ConsensusMessageControlKind::PayloadChunk => (
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Value::from(0_u64),
                Vec::new(),
            ),
            ConsensusMessageControlKind::CertifiedBodyResponse => (
                Value::from(9_u64),
                Value::from(2_u64),
                subject_value,
                Value::Null,
                Value::Null,
                Vec::new(),
            ),
            ConsensusMessageControlKind::CommitCertificateRequest => (
                Value::from(9_u64),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null,
                Vec::new(),
            ),
        };
        object_value([
            ("authenticated_via", Value::from(peer.clone())),
            (
                "block_hash",
                if subject.is_null() {
                    Value::Null
                } else {
                    Value::from(descriptor_subject().block_hash.to_string())
                },
            ),
            ("certificate_signers", Value::Array(certificate_signers)),
            ("cited_responder", cited_responder),
            (
                "envelope_digest",
                Value::from(CryptoHash::new(kind.as_str().as_bytes()).to_string()),
            ),
            ("execution_commitment", execution),
            ("height", height),
            ("kind", Value::from(kind.as_str())),
            ("sender", Value::from(peer)),
            ("sequence", Value::from(1_u64)),
            ("signer", signer),
            ("size_bytes", Value::from(64_u64)),
            ("subject", subject),
            ("view", view),
        ])
    }

    #[test]
    fn rule_constructors_bind_semantic_sender_and_authenticated_relay() {
        let sender = descriptor_peer();
        let relay = PeerId::new(
            KeyPair::try_from_seed(vec![0x44; 32], Algorithm::Ed25519)
                .expect("deterministic relay peer")
                .public_key()
                .clone(),
        );
        let direct = ConsensusMessageControlRule::exact(
            sender.clone(),
            ConsensusMessageControlKind::CommitVote,
            9,
            2,
            ConsensusMessageControlAction::Hold,
        );
        assert_eq!(direct.authenticated_via, sender);

        let relayed = ConsensusMessageControlRule::relayed(
            direct.sender.clone(),
            relay.clone(),
            direct.kind,
            direct.height,
            direct.view,
            direct.action,
        );
        assert_eq!(relayed.sender, direct.sender);
        assert_eq!(relayed.authenticated_via, relay);
        let encoded = rule_value(&relayed);
        let via_literal = relayed.authenticated_via.to_string();
        assert_eq!(
            encoded.get("authenticated_via").and_then(Value::as_str),
            Some(via_literal.as_str())
        );

        let second_relay = PeerId::new(
            KeyPair::try_from_seed(vec![0x45; 32], Algorithm::Ed25519)
                .expect("second deterministic relay peer")
                .public_key()
                .clone(),
        );
        let second_route = ConsensusMessageControlRule::relayed(
            relayed.sender.clone(),
            second_relay,
            relayed.kind,
            relayed.height,
            relayed.view,
            relayed.action,
        );
        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("controller");
        assert!(
            control
                .write_command(2, &[relayed.clone(), second_route], &[], 2, false)
                .is_ok(),
            "the same semantic rule through independent authenticated relays is unambiguous"
        );
        assert!(
            control
                .write_command(3, &[relayed.clone(), relayed], &[], 2, false)
                .is_err(),
            "the same semantic and authenticated rule still overlaps"
        );
    }

    #[test]
    fn writer_rejects_duplicate_and_reordered_release_sequences() {
        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("create control");
        assert!(control.write_command(2, &[], &[1, 1], 2, false).is_err());
        assert!(control.write_command(2, &[], &[2, 1], 2, false).is_err());
    }

    #[test]
    fn canonical_writer_has_explicit_version_and_bounds() {
        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("create control");
        let bytes = fs::read(control.root.join(CONTROL_FILE)).expect("read command");
        let value: Value = norito::json::from_slice(&bytes).expect("parse command");
        assert_eq!(
            value.get("version").and_then(Value::as_u64),
            Some(FORMAT_VERSION)
        );
        assert_eq!(value.get("drain").and_then(Value::as_bool), Some(false));
        assert!(
            control
                .write_command(2, &[], &[], MAX_HOLDS + 1, false)
                .is_err()
        );
    }

    #[test]
    fn native_amx_fault_command_and_ack_bind_exact_phase_source_and_revision() {
        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("controller");
        let source_id = [0xA7; 32];
        let revision = control
            .arm_native_amx_fault(NativeAmxFaultPhase::AfterCommitQc, source_id)
            .expect("arm exact fault");
        assert_eq!(revision, 1);
        let command =
            fs::read(control.root.join(NATIVE_AMX_FAULT_COMMAND_FILE)).expect("read fault command");
        let parsed = parse_native_amx_fault(&command).expect("command is valid ack shape");
        assert_eq!(
            parsed,
            NativeAmxFaultAck {
                revision,
                phase: NativeAmxFaultPhase::AfterCommitQc,
                source_id,
            }
        );

        write_atomic_private_file(
            &control.root,
            NATIVE_AMX_FAULT_ACK_FILE,
            &command,
            control.root_identity.owner,
        )
        .expect("write simulated daemon acknowledgement");
        assert_eq!(
            control
                .read_native_amx_fault_ack()
                .expect("read exact acknowledgement"),
            parsed
        );

        let mut noncanonical: Value =
            norito::json::from_slice(&command).expect("parse command for mutation");
        noncanonical
            .as_object_mut()
            .expect("fault command object")
            .insert(
                "source_id".to_owned(),
                Value::from(lowercase_hex(&source_id).to_ascii_uppercase()),
            );
        let uppercase = canonical_json(&noncanonical).expect("encode uppercase source");
        assert!(parse_native_amx_fault(&uppercase).is_err());
    }

    #[test]
    fn staged_initial_rules_replace_revision_one_before_startup() {
        let parent = tempdir().expect("temporary parent");
        let mut control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("create control");
        let key = KeyPair::try_from_seed(vec![9_u8; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        let rule = ConsensusMessageControlRule::exact(
            PeerId::new(key.public_key().clone()),
            ConsensusMessageControlKind::CommitVote,
            2,
            0,
            ConsensusMessageControlAction::Drop,
        );

        control
            .stage_initial_rules(std::slice::from_ref(&rule), 17)
            .expect("stage initial rule");

        let bytes = fs::read(control.root.join(CONTROL_FILE)).expect("read staged command");
        let value: Value = norito::json::from_slice(&bytes).expect("parse staged command");
        assert_eq!(value.get("revision").and_then(Value::as_u64), Some(1));
        assert_eq!(
            value.get("queue_capacity").and_then(Value::as_u64),
            Some(17)
        );
        assert_eq!(
            value.get("rules").and_then(Value::as_array).map(Vec::len),
            Some(1)
        );
        assert_eq!(control.initial_command.rules, vec![rule]);
        assert_eq!(control.initial_command.queue_capacity, 17);
        assert_eq!(
            control.initial_command.command_digest,
            CryptoHash::new(&bytes)
        );
        assert_eq!(
            *control
                .next_revision
                .lock()
                .expect("revision lock is healthy"),
            1
        );
        assert!(control.stage_initial_rules(&[], 17).is_err());
    }

    fn empty_ack(digest: CryptoHash) -> ConsensusMessageControlAck {
        ConsensusMessageControlAck {
            revision: 2,
            command_digest: digest,
            rules: Vec::new(),
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            held: Vec::new(),
            held_bytes: 0,
            release_pending: Vec::new(),
            in_flight: None,
            in_flight_bytes: 0,
            delivered: Vec::new(),
            retired: Vec::new(),
            dropped: 0,
            overflowed: 0,
            rejected_commands: 0,
            last_error: None,
            fatal: false,
            draining: false,
            drain_fence: None,
        }
    }

    #[test]
    fn exact_ack_binding_rejects_higher_revision_digest_rules_and_capacity_mismatch() {
        let digest = CryptoHash::new(b"expected");
        let expected = ExpectedAck {
            revision: 2,
            command_digest: digest,
            rules: &[],
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            drain: false,
        };
        let ack = empty_ack(digest);
        assert!(ack_matches_expected(&ack, &expected));
        assert!(!ack_matches_expected_release_in_progress(&ack, &expected));
        assert!(ack_matches_expected_release_in_progress(
            &ConsensusMessageControlAck {
                release_pending: vec![1],
                ..ack.clone()
            },
            &expected
        ));
        assert!(ack_matches_expected_release_in_progress(
            &ConsensusMessageControlAck {
                in_flight: Some(1),
                in_flight_bytes: 1,
                ..ack.clone()
            },
            &expected
        ));
        assert!(!ack_matches_expected(
            &ConsensusMessageControlAck {
                revision: 3,
                ..ack.clone()
            },
            &expected
        ));
        let drain_expected = ExpectedAck {
            revision: 2,
            command_digest: digest,
            rules: &[],
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
            drain: true,
        };
        assert!(!ack_matches_expected(&ack, &drain_expected));
        assert!(ack_matches_expected(
            &ConsensusMessageControlAck {
                drain_fence: Some(2),
                ..ack.clone()
            },
            &drain_expected
        ));
        assert!(!ack_matches_expected(
            &ConsensusMessageControlAck {
                command_digest: CryptoHash::new(b"other"),
                ..ack.clone()
            },
            &expected
        ));
        let key = KeyPair::try_from_seed(vec![7_u8; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        assert!(!ack_matches_expected(
            &ConsensusMessageControlAck {
                rules: vec![ConsensusMessageControlRule::exact(
                    PeerId::new(key.public_key().clone()),
                    ConsensusMessageControlKind::CommitVote,
                    2,
                    0,
                    ConsensusMessageControlAction::Hold,
                )],
                ..ack.clone()
            },
            &expected
        ));
        assert!(!ack_matches_expected(
            &ConsensusMessageControlAck {
                queue_capacity: 1,
                ..ack
            },
            &expected
        ));
    }

    #[tokio::test]
    async fn controller_operations_are_serialized() {
        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("create control");
        let first = control.operation.lock().await;
        assert!(control.operation.try_lock().is_err());
        drop(first);
        assert!(control.operation.try_lock().is_ok());
    }

    #[test]
    fn ack_parser_rejects_unknown_schema_and_nonfatal_overflow() {
        let digest = CryptoHash::new(b"command");
        let base = object_value([
            ("command_digest", Value::from(digest.to_string())),
            ("delivered", Value::Array(Vec::new())),
            ("dropped", Value::from(0_u64)),
            ("drain_fence", Value::Null),
            ("draining", Value::from(false)),
            ("fatal", Value::from(false)),
            ("held", Value::Array(Vec::new())),
            ("held_bytes", Value::from(0_u64)),
            ("in_flight", Value::Null),
            ("in_flight_bytes", Value::from(0_u64)),
            ("last_error", Value::Null),
            ("overflowed", Value::from(0_u64)),
            ("queue_capacity", Value::from(DEFAULT_QUEUE_CAPACITY as u64)),
            ("rejected_commands", Value::from(0_u64)),
            ("release_pending", Value::Array(Vec::new())),
            ("retired", Value::Array(Vec::new())),
            ("revision", Value::from(1_u64)),
            ("rules", Value::Array(Vec::new())),
            ("version", Value::from(FORMAT_VERSION)),
        ]);
        let bytes = canonical_json(&base).expect("canonical ack");
        assert!(parse_ack(&bytes).is_ok());

        let mut unknown = base.clone();
        unknown
            .as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), Value::from(1_u64));
        assert!(parse_ack(&canonical_json(&unknown).expect("canonical unknown ack")).is_err());

        let mut overflow = base;
        overflow
            .as_object_mut()
            .expect("object")
            .insert("overflowed".to_owned(), Value::from(1_u64));
        assert!(parse_ack(&canonical_json(&overflow).expect("canonical overflow ack")).is_err());
    }

    #[test]
    fn ack_parser_enforces_terminal_disjointness_and_in_flight_capacity() {
        let digest = CryptoHash::new(b"terminal-command");
        let empty = || {
            object_value([
                ("command_digest", Value::from(digest.to_string())),
                ("delivered", Value::Array(Vec::new())),
                ("dropped", Value::from(0_u64)),
                ("drain_fence", Value::Null),
                ("draining", Value::from(false)),
                ("fatal", Value::from(false)),
                ("held", Value::Array(Vec::new())),
                ("held_bytes", Value::from(0_u64)),
                ("in_flight", Value::Null),
                ("in_flight_bytes", Value::from(0_u64)),
                ("last_error", Value::Null),
                ("overflowed", Value::from(0_u64)),
                ("queue_capacity", Value::from(2_u64)),
                ("rejected_commands", Value::from(0_u64)),
                ("release_pending", Value::Array(Vec::new())),
                ("retired", Value::Array(Vec::new())),
                ("revision", Value::from(1_u64)),
                ("rules", Value::Array(Vec::new())),
                ("version", Value::from(FORMAT_VERSION)),
            ])
        };
        let parse = |value: &Value| parse_ack(&canonical_json(value).expect("canonical ack"));

        let mut retired = empty();
        retired.as_object_mut().expect("ack object").insert(
            "retired".to_owned(),
            Value::Array(vec![Value::from(1_u64), Value::from(2_u64)]),
        );
        let parsed = parse(&retired).expect("sorted positive retirement is valid");
        assert_eq!(parsed.retired, vec![1, 2]);
        assert!(parsed.delivered.is_empty());

        for invalid in [
            vec![Value::from(0_u64)],
            vec![Value::from(2_u64), Value::from(1_u64)],
            vec![Value::from(1_u64), Value::from(1_u64)],
        ] {
            let mut ack = empty();
            ack.as_object_mut()
                .expect("ack object")
                .insert("retired".to_owned(), Value::Array(invalid));
            assert!(parse(&ack).is_err());
        }

        let mut terminal_overlap = empty();
        let object = terminal_overlap.as_object_mut().expect("ack object");
        object.insert(
            "delivered".to_owned(),
            Value::Array(vec![Value::from(1_u64)]),
        );
        object.insert("retired".to_owned(), Value::Array(vec![Value::from(1_u64)]));
        assert!(parse(&terminal_overlap).is_err());

        let mut valid_in_flight = empty();
        let object = valid_in_flight.as_object_mut().expect("ack object");
        object.insert("in_flight".to_owned(), Value::from(1_u64));
        object.insert("in_flight_bytes".to_owned(), Value::from(7_u64));
        let parsed = parse(&valid_in_flight).expect("in-flight bytes are explicitly retained");
        assert_eq!(parsed.in_flight, Some(1));
        assert_eq!(parsed.in_flight_bytes, 7);

        let mut in_flight_overlap = valid_in_flight;
        let object = in_flight_overlap.as_object_mut().expect("ack object");
        object.insert("retired".to_owned(), Value::Array(vec![Value::from(1_u64)]));
        assert!(parse(&in_flight_overlap).is_err());

        let held = held_descriptor(ConsensusMessageControlKind::PayloadChunk);
        let mut held_overlap = empty();
        let object = held_overlap.as_object_mut().expect("ack object");
        object.insert("held".to_owned(), Value::Array(vec![held.clone()]));
        object.insert("held_bytes".to_owned(), Value::from(64_u64));
        object.insert(
            "release_pending".to_owned(),
            Value::Array(vec![Value::from(1_u64)]),
        );
        object.insert("retired".to_owned(), Value::Array(vec![Value::from(1_u64)]));
        assert!(parse(&held_overlap).is_err());

        let mut over_count = empty();
        let object = over_count.as_object_mut().expect("ack object");
        object.insert("held".to_owned(), Value::Array(vec![held.clone()]));
        object.insert("held_bytes".to_owned(), Value::from(64_u64));
        object.insert("in_flight".to_owned(), Value::from(2_u64));
        object.insert("in_flight_bytes".to_owned(), Value::from(1_u64));
        object.insert("queue_capacity".to_owned(), Value::from(1_u64));
        assert!(parse(&over_count).is_err());

        let mut over_bytes = empty();
        let object = over_bytes.as_object_mut().expect("ack object");
        object.insert("held".to_owned(), Value::Array(vec![held]));
        object.insert("held_bytes".to_owned(), Value::from(64_u64));
        object.insert("in_flight".to_owned(), Value::from(2_u64));
        object.insert("in_flight_bytes".to_owned(), Value::from(MAX_HELD_BYTES));
        assert!(parse(&over_bytes).is_err());

        let mut missing_bytes = empty();
        missing_bytes
            .as_object_mut()
            .expect("ack object")
            .insert("in_flight".to_owned(), Value::from(1_u64));
        assert!(parse(&missing_bytes).is_err());

        let mut orphan_bytes = empty();
        orphan_bytes
            .as_object_mut()
            .expect("ack object")
            .insert("in_flight_bytes".to_owned(), Value::from(1_u64));
        assert!(parse(&orphan_bytes).is_err());
    }

    #[test]
    fn ack_parser_models_chunk_rounds_as_absent_and_rejects_fabrication() {
        let key = KeyPair::try_from_seed(vec![17_u8; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        let sender = PeerId::new(key.public_key().clone()).to_string();
        let digest = CryptoHash::new(b"command");
        let chunk = object_value([
            ("authenticated_via", Value::from(sender.clone())),
            ("block_hash", Value::Null),
            ("certificate_signers", Value::Array(Vec::new())),
            ("cited_responder", Value::Null),
            (
                "envelope_digest",
                Value::from(CryptoHash::new(b"chunk-envelope").to_string()),
            ),
            ("execution_commitment", Value::Null),
            ("height", Value::Null),
            ("kind", Value::from("payload_chunk")),
            ("sender", Value::from(sender)),
            ("sequence", Value::from(1_u64)),
            ("signer", Value::from(0_u64)),
            ("size_bytes", Value::from(64_u64)),
            ("subject", Value::Null),
            ("view", Value::Null),
        ]);
        let ack = |held: Value| {
            object_value([
                ("command_digest", Value::from(digest.to_string())),
                ("delivered", Value::Array(Vec::new())),
                ("dropped", Value::from(0_u64)),
                ("drain_fence", Value::from(1_u64)),
                ("draining", Value::from(true)),
                ("fatal", Value::from(false)),
                ("held", Value::Array(vec![held])),
                ("held_bytes", Value::from(64_u64)),
                ("in_flight", Value::Null),
                ("in_flight_bytes", Value::from(0_u64)),
                ("last_error", Value::Null),
                ("overflowed", Value::from(0_u64)),
                ("queue_capacity", Value::from(DEFAULT_QUEUE_CAPACITY as u64)),
                ("rejected_commands", Value::from(0_u64)),
                ("release_pending", Value::Array(vec![Value::from(1_u64)])),
                ("retired", Value::Array(Vec::new())),
                ("revision", Value::from(1_u64)),
                ("rules", Value::Array(Vec::new())),
                ("version", Value::from(FORMAT_VERSION)),
            ])
        };
        let parsed =
            parse_ack(&canonical_json(&ack(chunk.clone())).expect("canonical ack")).expect("ack");
        assert_eq!(parsed.held[0].sender, parsed.held[0].authenticated_via);
        assert_eq!(parsed.held[0].signer, Some(0));
        assert!(parsed.held[0].subject.is_none());
        assert!(parsed.held[0].execution_commitment.is_none());

        let mut relayed = chunk.clone();
        relayed.as_object_mut().expect("chunk descriptor").insert(
            "authenticated_via".to_owned(),
            Value::from(
                PeerId::new(
                    KeyPair::try_from_seed(vec![18_u8; 32], Algorithm::Ed25519)
                        .expect("second deterministic peer key")
                        .public_key()
                        .clone(),
                )
                .to_string(),
            ),
        );
        let parsed_relayed =
            parse_ack(&canonical_json(&ack(relayed)).expect("canonical relayed ack"))
                .expect("trusted relay identity remains explicit and valid");
        assert_ne!(
            parsed_relayed.held[0].sender,
            parsed_relayed.held[0].authenticated_via
        );

        let mut missing_signer = chunk.clone();
        missing_signer
            .as_object_mut()
            .expect("chunk descriptor")
            .insert("signer".to_owned(), Value::Null);
        assert!(
            parse_ack(&canonical_json(&ack(missing_signer)).expect("canonical missing-signer ack"))
                .is_err()
        );

        let mut inconsistent_subject = chunk.clone();
        inconsistent_subject
            .as_object_mut()
            .expect("chunk descriptor")
            .insert(
                "block_hash".to_owned(),
                Value::from(CryptoHash::new(b"unbound-block").to_string()),
            );
        assert!(
            parse_ack(
                &canonical_json(&ack(inconsistent_subject))
                    .expect("canonical inconsistent-subject ack")
            )
            .is_err()
        );

        let mut malformed_digest = chunk.clone();
        malformed_digest
            .as_object_mut()
            .expect("chunk descriptor")
            .insert(
                "envelope_digest".to_owned(),
                Value::from("not-a-canonical-hash"),
            );
        assert!(
            parse_ack(
                &canonical_json(&ack(malformed_digest)).expect("canonical malformed-digest ack")
            )
            .is_err()
        );

        let mut subjectless_vote = chunk.clone();
        {
            let subjectless_vote = subjectless_vote.as_object_mut().expect("chunk descriptor");
            subjectless_vote.insert("height".to_owned(), Value::from(9_u64));
            subjectless_vote.insert("kind".to_owned(), Value::from("prepare_vote"));
            subjectless_vote.insert("view".to_owned(), Value::from(0_u64));
        }
        assert!(
            parse_ack(
                &canonical_json(&ack(subjectless_vote)).expect("canonical subjectless-vote ack")
            )
            .is_err()
        );

        let mut timeout_certificate = chunk.clone();
        let timeout_certificate_object = timeout_certificate
            .as_object_mut()
            .expect("chunk descriptor");
        timeout_certificate_object.insert(
            "certificate_signers".to_owned(),
            Value::Array(vec![
                Value::from(0_u64),
                Value::from(1_u64),
                Value::from(2_u64),
            ]),
        );
        timeout_certificate_object.insert("height".to_owned(), Value::from(9_u64));
        timeout_certificate_object.insert("kind".to_owned(), Value::from("timeout_certificate"));
        timeout_certificate_object.insert("signer".to_owned(), Value::Null);
        timeout_certificate_object.insert("view".to_owned(), Value::from(0_u64));
        assert!(
            parse_ack(
                &canonical_json(&ack(timeout_certificate.clone()))
                    .expect("canonical timeout-certificate ack")
            )
            .is_ok()
        );

        let mut signerless_certificate = timeout_certificate.clone();
        signerless_certificate
            .as_object_mut()
            .expect("timeout-certificate descriptor")
            .insert("certificate_signers".to_owned(), Value::Array(Vec::new()));
        assert!(
            parse_ack(
                &canonical_json(&ack(signerless_certificate))
                    .expect("canonical signerless-certificate ack")
            )
            .is_err()
        );

        let mut duplicate_certificate_signer = timeout_certificate;
        duplicate_certificate_signer
            .as_object_mut()
            .expect("timeout-certificate descriptor")
            .insert(
                "certificate_signers".to_owned(),
                Value::Array(vec![Value::from(0_u64), Value::from(0_u64)]),
            );
        assert!(
            parse_ack(
                &canonical_json(&ack(duplicate_certificate_signer))
                    .expect("canonical duplicate-certificate-signer ack")
            )
            .is_err()
        );

        let mut fabricated = chunk;
        fabricated
            .as_object_mut()
            .expect("chunk descriptor")
            .insert("height".to_owned(), Value::from(9_u64));
        assert!(
            parse_ack(&canonical_json(&ack(fabricated)).expect("canonical invalid ack")).is_err()
        );
    }

    #[test]
    fn held_descriptor_parser_accepts_every_daemon_payload_shape() {
        for kind in [
            ConsensusMessageControlKind::Proposal,
            ConsensusMessageControlKind::PrepareVote,
            ConsensusMessageControlKind::CommitVote,
            ConsensusMessageControlKind::PrepareCertificate,
            ConsensusMessageControlKind::CommitCertificate,
            ConsensusMessageControlKind::TimeoutVote,
            ConsensusMessageControlKind::TimeoutCertificate,
            ConsensusMessageControlKind::PayloadManifest,
            ConsensusMessageControlKind::PayloadChunk,
            ConsensusMessageControlKind::CertifiedBodyRequest,
            ConsensusMessageControlKind::CertifiedBodyResponse,
            ConsensusMessageControlKind::CommitCertificateRequest,
            ConsensusMessageControlKind::CommitCertificateResponse,
        ] {
            let parsed = parse_held(&held_descriptor(kind))
                .unwrap_or_else(|error| panic!("daemon {kind:?} descriptor failed: {error:#}"));
            assert_eq!(parsed.kind, kind);
            assert_eq!(parsed.sender, parsed.authenticated_via);
            assert_ne!(parsed.envelope_digest, CryptoHash::new(b""));
            if kind == ConsensusMessageControlKind::CertifiedBodyResponse {
                assert_eq!(parsed.signer, None);
                assert_eq!(parsed.cited_responder, Some(0));
            } else {
                assert_eq!(parsed.cited_responder, None);
            }
        }
    }

    #[test]
    fn held_descriptor_parser_rejects_invalid_execution_and_certificate_shapes() {
        let mut invalid_execution = held_descriptor(ConsensusMessageControlKind::PrepareVote);
        invalid_execution
            .as_object_mut()
            .expect("vote descriptor")
            .get_mut("execution_commitment")
            .and_then(Value::as_object_mut)
            .expect("execution commitment")
            .insert("topup_anchor_count".to_owned(), Value::from(1_u64));
        assert!(parse_held(&invalid_execution).is_err());

        let mut reordered = held_descriptor(ConsensusMessageControlKind::PrepareCertificate);
        reordered
            .as_object_mut()
            .expect("certificate descriptor")
            .insert(
                "certificate_signers".to_owned(),
                Value::Array(vec![Value::from(1_u64), Value::from(0_u64)]),
            );
        assert!(parse_held(&reordered).is_err());

        let mut missing_cited_responder =
            held_descriptor(ConsensusMessageControlKind::CertifiedBodyResponse);
        missing_cited_responder
            .as_object_mut()
            .expect("certified response descriptor")
            .insert("cited_responder".to_owned(), Value::Null);
        assert!(parse_held(&missing_cited_responder).is_err());

        let mut false_response_signer =
            held_descriptor(ConsensusMessageControlKind::CertifiedBodyResponse);
        false_response_signer
            .as_object_mut()
            .expect("certified response descriptor")
            .insert("signer".to_owned(), Value::from(0_u64));
        assert!(parse_held(&false_response_signer).is_err());

        let mut spurious_cited_responder =
            held_descriptor(ConsensusMessageControlKind::PrepareVote);
        spurious_cited_responder
            .as_object_mut()
            .expect("vote descriptor")
            .insert("cited_responder".to_owned(), Value::from(0_u64));
        assert!(parse_held(&spurious_cited_responder).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn acknowledgement_reader_rejects_symlink_and_hardlink_sources() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let parent = tempdir().expect("temporary parent");
        let control =
            ConsensusMessageControl::create(parent.path().join("control")).expect("create control");
        let source = control.root.join("source");
        fs::write(&source, b"{}").expect("write source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).expect("chmod source");
        let hardlink = control.root.join("hardlink");
        fs::hard_link(&source, &hardlink).expect("hardlink source");
        assert!(
            read_bounded_private_file(&hardlink, MAX_ACK_BYTES, control.root_identity.owner)
                .is_err()
        );
        let symlink_path = control.root.join("symlink");
        symlink(&source, &symlink_path).expect("symlink source");
        assert!(
            read_bounded_private_file(&symlink_path, MAX_ACK_BYTES, control.root_identity.owner)
                .is_err()
        );
    }
}
