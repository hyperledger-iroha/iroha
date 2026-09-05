//! Test-network-only inbound Sumeragi v2 message controller.
//!
//! This module is compiled only for the dedicated adversarial-test daemon.
//! It never changes transport authentication or wire encryption: rules are
//! evaluated after the P2P layer has authenticated the remote peer and before
//! the message enters ordinary relay accounting.
#[cfg(not(unix))]
compile_error!(
    "the test-network message controller requires Unix openat/no-follow and ownership semantics"
);
use iroha_core::NetworkMessage;
use iroha_crypto::Hash;
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{
            BlockSubject, ConsensusMessageV2Payload, ExecutionCommitment, GlobalPhase,
            MAX_VALIDATORS_PER_HEIGHT, PayloadManifest, ValidatorIndex,
        },
    },
    peer::{Peer, PeerId},
};
use iroha_p2p::network::NetworkReplyRoute;
use norito::{
    codec::Encode,
    json::{Map, Value},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
/// Environment variable consumed only by the feature-isolated test daemon.
pub(crate) const CONTROL_DIR_ENV: &str = "IROHA_TEST_CONSENSUS_MESSAGE_CONTROL_DIR";
const CONTROL_FILE: &str = "command.norito.json";
const ACK_FILE: &str = "ack.norito.json";
const FORMAT_VERSION: u64 = 5;
const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_ACK_BYTES: usize = 1024 * 1024;
const MAX_RULES: usize = 256;
const MAX_HOLDS: usize = 1_024;
const MAX_HELD_BYTES: usize = 64 * 1024 * 1024;
const MAX_RELEASES: usize = MAX_HOLDS;
/// Bound pre-consensus Proposal evidence independently of message volume.
const MAX_PROPOSAL_ROUND_EVIDENCE: usize = MAX_HOLDS;
const MAX_SENDER_BYTES: usize = 256;
const MAX_KIND_BYTES: usize = 64;
const MAX_HASH_BYTES: usize = 128;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Drop,
    Hold,
}
impl Action {
    fn parse(value: &Value) -> Result<Self, ControlError> {
        match value.as_str() {
            Some("drop") => Ok(Self::Drop),
            Some("hold") => Ok(Self::Hold),
            _ => Err(ControlError::InvalidField("action")),
        }
    }
    const fn as_str(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Hold => "hold",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MessageKind {
    Proposal,
    PrepareVote,
    CommitVote,
    PrepareCertificate,
    CommitCertificate,
    TimeoutVote,
    TimeoutCertificate,
    PayloadChunk,
    CertifiedBodyRequest,
    CertifiedBodyResponse,
    CommitCertificateRequest,
    CommitCertificateResponse,
    GlobalBeaconPartialSignature,
}
impl MessageKind {
    fn parse(value: &Value) -> Result<Self, ControlError> {
        let Some(value) = value.as_str() else {
            return Err(ControlError::InvalidField("kind"));
        };
        if value.len() > MAX_KIND_BYTES {
            return Err(ControlError::FieldTooLarge("kind"));
        }
        match value {
            "proposal" => Ok(Self::Proposal),
            "prepare_vote" => Ok(Self::PrepareVote),
            "commit_vote" => Ok(Self::CommitVote),
            "prepare_certificate" => Ok(Self::PrepareCertificate),
            "commit_certificate" => Ok(Self::CommitCertificate),
            "timeout_vote" => Ok(Self::TimeoutVote),
            "timeout_certificate" => Ok(Self::TimeoutCertificate),
            "payload_chunk" => Ok(Self::PayloadChunk),
            "certified_body_request" => Ok(Self::CertifiedBodyRequest),
            "certified_body_response" => Ok(Self::CertifiedBodyResponse),
            "commit_certificate_request" => Ok(Self::CommitCertificateRequest),
            "commit_certificate_response" => Ok(Self::CommitCertificateResponse),
            "global_beacon_partial_signature" => Ok(Self::GlobalBeaconPartialSignature),
            _ => Err(ControlError::InvalidField("kind")),
        }
    }
    const fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::PrepareVote => "prepare_vote",
            Self::CommitVote => "commit_vote",
            Self::PrepareCertificate => "prepare_certificate",
            Self::CommitCertificate => "commit_certificate",
            Self::TimeoutVote => "timeout_vote",
            Self::TimeoutCertificate => "timeout_certificate",
            Self::PayloadChunk => "payload_chunk",
            Self::CertifiedBodyRequest => "certified_body_request",
            Self::CertifiedBodyResponse => "certified_body_response",
            Self::CommitCertificateRequest => "commit_certificate_request",
            Self::CommitCertificateResponse => "commit_certificate_response",
            Self::GlobalBeaconPartialSignature => "global_beacon_partial_signature",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct MessageMeta {
    sender: PeerId,
    authenticated_via: PeerId,
    kind: MessageKind,
    height: Option<u64>,
    view: Option<u64>,
    block_hash: Option<HashOf<BlockHeader>>,
    manifest_hash: Option<HashOf<PayloadManifest>>,
    chunk_index: Option<u32>,
    subject: Option<BlockSubject>,
    execution_commitment: Option<ExecutionCommitment>,
    signer: Option<ValidatorIndex>,
    cited_responder: Option<PeerId>,
    certificate_signers: Vec<ValidatorIndex>,
    envelope_digest: Hash,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    sender: PeerId,
    authenticated_via: PeerId,
    kind: MessageKind,
    height: Option<u64>,
    view: Option<u64>,
    block_hash: Option<HashOf<BlockHeader>>,
    manifest_hash: Option<HashOf<PayloadManifest>>,
    chunk_index: Option<u32>,
    proposal_height: Option<u64>,
    proposal_view: Option<u64>,
    action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ProposalManifestRoute {
    sender: PeerId,
    authenticated_via: PeerId,
    manifest_hash: HashOf<PayloadManifest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProposalRound {
    height: u64,
    view: u64,
}

impl Rule {
    fn matches(&self, meta: &MessageMeta) -> bool {
        self.sender == meta.sender
            && self.authenticated_via == meta.authenticated_via
            && self.kind == meta.kind
            && self.height == meta.height
            && self.view == meta.view
            && self
                .block_hash
                .as_ref()
                .is_none_or(|expected| meta.block_hash.as_ref() == Some(expected))
            && (self.kind != MessageKind::PayloadChunk
                || self.chunk_index == meta.chunk_index
                    && self
                        .manifest_hash
                        .as_ref()
                        .is_none_or(|expected| meta.manifest_hash.as_ref() == Some(expected)))
    }

    fn overlaps(&self, other: &Self) -> bool {
        if self.sender != other.sender
            || self.authenticated_via != other.authenticated_via
            || self.kind != other.kind
        {
            return false;
        }
        if self.kind == MessageKind::PayloadChunk {
            if self.chunk_index != other.chunk_index {
                return false;
            }
            if let ((Some(left_height), Some(left_view)), (Some(right_height), Some(right_view))) = (
                (self.proposal_height, self.proposal_view),
                (other.proposal_height, other.proposal_view),
            ) && (left_height, left_view) != (right_height, right_view)
            {
                // Two unresolved selectors are both Holds and Proposal
                // evidence later partitions them by exact round. A resolved
                // selector already matches by manifest alone, however, so
                // preserve ambiguity for an equal manifest or a mixed-action
                // provisional match.
                return match (self.manifest_hash, other.manifest_hash) {
                    (None, None) => false,
                    (Some(left), Some(right)) => left == right,
                    (None, Some(_)) | (Some(_), None) => self.action != other.action,
                };
            }
            return match (self.manifest_hash, other.manifest_hash) {
                (Some(left), Some(right))
                    if self.proposal_height.is_none() && other.proposal_height.is_none() =>
                {
                    left == right
                }
                _ => true,
            };
        }
        self.height == other.height
            && self.view == other.view
            && (self.block_hash.is_none()
                || other.block_hash.is_none()
                || self.block_hash == other.block_hash)
    }
}

fn rule_matches_with_proposal_evidence(
    rule: &Rule,
    meta: &MessageMeta,
    proposal_round_evidence: &BTreeMap<ProposalManifestRoute, ProposalRound>,
) -> bool {
    let matches = rule.matches(meta);
    if !matches || rule.kind != MessageKind::PayloadChunk || rule.manifest_hash.is_some() {
        return matches;
    }
    let (Some(manifest_hash), Some(height), Some(view)) =
        (meta.manifest_hash, rule.proposal_height, rule.proposal_view)
    else {
        return true;
    };
    let route = ProposalManifestRoute {
        sender: meta.sender.clone(),
        authenticated_via: meta.authenticated_via.clone(),
        manifest_hash,
    };
    proposal_round_evidence
        .get(&route)
        .is_none_or(|round| *round == (ProposalRound { height, view }))
}

fn record_proposal_round_evidence<R, O>(
    state: &mut State<R, O>,
    meta: &MessageMeta,
    height: u64,
    view: u64,
    manifest_hash: HashOf<PayloadManifest>,
) -> Result<bool, ControlError> {
    let route = ProposalManifestRoute {
        sender: meta.sender.clone(),
        authenticated_via: meta.authenticated_via.clone(),
        manifest_hash,
    };
    let round = ProposalRound { height, view };
    if let Some(existing) = state.proposal_round_evidence.get(&route) {
        return if *existing == round {
            Ok(false)
        } else {
            Err(ControlError::InvalidMessageDescriptor)
        };
    }
    while state.proposal_round_evidence.len() >= MAX_PROPOSAL_ROUND_EVIDENCE {
        let Some(oldest) = state.proposal_round_evidence_order.pop_front() else {
            state.proposal_round_evidence.clear();
            break;
        };
        state.proposal_round_evidence.remove(&oldest);
    }
    state.proposal_round_evidence.insert(route.clone(), round);
    state.proposal_round_evidence_order.push_back(route);
    Ok(true)
}

fn resolve_deferred_rules_for_proposal(
    rules: &mut [Rule],
    sender: &PeerId,
    authenticated_via: &PeerId,
    height: u64,
    view: u64,
    manifest_hash: HashOf<PayloadManifest>,
) -> Result<bool, ControlError> {
    let mut resolved = false;
    for rule in rules.iter_mut().filter(|rule| {
        rule.kind == MessageKind::PayloadChunk
            && &rule.sender == sender
            && &rule.authenticated_via == authenticated_via
            && rule.proposal_height == Some(height)
            && rule.proposal_view == Some(view)
    }) {
        match rule.manifest_hash {
            None => {
                rule.manifest_hash = Some(manifest_hash);
                resolved = true;
            }
            Some(existing) if existing != manifest_hash => {
                return Err(ControlError::InvalidMessageDescriptor);
            }
            Some(_) => {}
        }
    }
    Ok(resolved)
}

fn resolve_deferred_rules_from_evidence(
    rules: &mut [Rule],
    proposal_round_evidence: &BTreeMap<ProposalManifestRoute, ProposalRound>,
) -> Result<bool, ControlError> {
    let mut resolved = false;
    for (route, round) in proposal_round_evidence {
        resolved |= resolve_deferred_rules_for_proposal(
            rules,
            &route.sender,
            &route.authenticated_via,
            round.height,
            round.view,
            route.manifest_hash,
        )?;
    }
    Ok(resolved)
}

fn resolve_deferred_chunk_rules<R, O>(
    state: &mut State<R, O>,
    meta: &MessageMeta,
) -> Result<bool, ControlError> {
    if meta.kind != MessageKind::Proposal {
        return Ok(false);
    }
    let (Some(height), Some(view), Some(manifest_hash)) =
        (meta.height, meta.view, meta.manifest_hash)
    else {
        return Err(ControlError::InvalidMessageDescriptor);
    };
    let evidence_changed =
        record_proposal_round_evidence(state, meta, height, view, manifest_hash)?;
    let mut resolved = resolve_deferred_rules_for_proposal(
        &mut state.rules,
        &meta.sender,
        &meta.authenticated_via,
        height,
        view,
        manifest_hash,
    )?;
    if let Some(next_rules) = state.drain_next_rules.as_mut() {
        resolved |= resolve_deferred_rules_for_proposal(
            next_rules,
            &meta.sender,
            &meta.authenticated_via,
            height,
            view,
            manifest_hash,
        )?;
    }
    let mut releases_changed = false;
    if (evidence_changed || resolved) && state.drain_next_rules.is_none() {
        let mismatched = state
            .held
            .iter()
            .filter_map(|(sequence, entry)| {
                (entry.descriptor.meta.kind == MessageKind::PayloadChunk
                    && !state.rules.iter().any(|rule| {
                        rule.action == Action::Hold
                            && rule_matches_with_proposal_evidence(
                                rule,
                                &entry.descriptor.meta,
                                &state.proposal_round_evidence,
                            )
                    }))
                .then_some(*sequence)
            })
            .collect::<Vec<_>>();
        let mut release_pending = state
            .release_pending
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let prior_len = release_pending.len();
        release_pending.extend(mismatched);
        releases_changed = release_pending.len() != prior_len;
        state.release_pending = release_pending.into_iter().collect();
    }
    Ok(resolved || releases_changed)
}
#[derive(Debug)]
struct Command {
    revision: u64,
    queue_capacity: usize,
    rules: Vec<Rule>,
    release: Vec<u64>,
    drain: bool,
}
#[derive(Clone, Debug)]
struct HeldDescriptor {
    sequence: u64,
    meta: MessageMeta,
    size_bytes: usize,
}
pub(crate) struct HeldMessage<R = NetworkReplyRoute, O = ()> {
    pub(crate) sequence: u64,
    pub(crate) peer: Peer,
    pub(crate) authenticated_via: PeerId,
    pub(crate) message: NetworkMessage,
    pub(crate) size_bytes: usize,
    pub(crate) reply_route: Option<R>,
    /// Exact local-only ownership retained with the controlled occurrence.
    pub(crate) ownership: Option<O>,
}
struct HeldEntry<R, O> {
    descriptor: HeldDescriptor,
    peer: Peer,
    authenticated_via: PeerId,
    message: NetworkMessage,
    reply_route: Option<R>,
    ownership: Option<O>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    Pass,
    /// The controller atomically retained the complete local occurrence.
    Held,
    Consumed,
}
/// Terminal disposition of one exact controlled release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReleaseOutcome {
    /// Ordinary ingress accepted, coalesced, or found the occurrence obsolete.
    Delivered,
    /// The exact reply authority retired before ordinary ingress succeeded.
    Retired,
    /// The release could not reach a successful terminal state.
    Failed,
}
struct State<R = NetworkReplyRoute, O = ()> {
    revision: u64,
    command_digest: Option<Hash>,
    last_seen_digest: Option<Hash>,
    rules: Vec<Rule>,
    // FIFO-bounded facts learned from Proposals on this exact authenticated
    // route. They disambiguate compact chunks, which carry no round fields.
    proposal_round_evidence: BTreeMap<ProposalManifestRoute, ProposalRound>,
    proposal_round_evidence_order: VecDeque<ProposalManifestRoute>,
    queue_capacity: usize,
    held: BTreeMap<u64, HeldEntry<R, O>>,
    held_bytes: usize,
    release_pending: VecDeque<u64>,
    in_flight: Option<u64>,
    in_flight_bytes: usize,
    delivered: Vec<u64>,
    retired: Vec<u64>,
    next_sequence: u64,
    dropped: u64,
    overflowed: u64,
    rejected_commands: u64,
    last_error: Option<String>,
    fatal: bool,
    drain_next_rules: Option<Vec<Rule>>,
    drain_fence: Option<u64>,
}
impl<R, O> Default for State<R, O> {
    fn default() -> Self {
        Self {
            revision: 0,
            command_digest: None,
            last_seen_digest: None,
            rules: Vec::new(),
            proposal_round_evidence: BTreeMap::new(),
            proposal_round_evidence_order: VecDeque::new(),
            queue_capacity: MAX_HOLDS,
            held: BTreeMap::new(),
            held_bytes: 0,
            release_pending: VecDeque::new(),
            in_flight: None,
            in_flight_bytes: 0,
            delivered: Vec::new(),
            retired: Vec::new(),
            next_sequence: 1,
            dropped: 0,
            overflowed: 0,
            rejected_commands: 0,
            last_error: None,
            fatal: false,
            drain_next_rules: None,
            drain_fence: None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RootIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}
/// Feature-only controller shared by relay workers and its file watcher.
pub(crate) struct Controller<R = NetworkReplyRoute, O = ()> {
    root: PathBuf,
    root_identity: RootIdentity,
    state: Mutex<State<R, O>>,
    ack_publish: Mutex<()>,
}
impl<O> Controller<NetworkReplyRoute, O> {
    /// Open and pin the explicitly configured private control directory.
    pub(crate) fn from_env() -> Result<Option<Self>, ControlError> {
        let Some(raw) = env::var_os(CONTROL_DIR_ENV) else {
            return Ok(None);
        };
        let root = PathBuf::from(raw);
        if !root.is_absolute() {
            return Err(ControlError::UnsafeRoot);
        }
        let canonical = root.canonicalize().map_err(ControlError::Io)?;
        if canonical != root {
            return Err(ControlError::UnsafeRoot);
        }
        let metadata = fs::symlink_metadata(&root).map_err(ControlError::Io)?;
        validate_private_root(&metadata)?;
        let root_identity = root_identity(&metadata);
        let controller = Self {
            root,
            root_identity,
            state: Mutex::new(State::default()),
            ack_publish: Mutex::new(()),
        };
        controller.validate_root()?;
        controller.poll_command()?;
        if controller
            .state
            .lock()
            .expect("message control state poisoned")
            .revision
            == 0
        {
            return Err(ControlError::ControllerUninitialized);
        }
        controller.publish_ack()?;
        Ok(Some(controller))
    }
}
impl<R, O> Controller<R, O> {
    /// Construct an isolated controller for daemon boundary tests.
    #[cfg(test)]
    pub(crate) fn for_tests() -> (tempfile::TempDir, Self) {
        use std::os::unix::fs::PermissionsExt;
        let parent = tempfile::tempdir().expect("temporary parent");
        let root = parent.path().join("control");
        fs::create_dir(&root).expect("create control root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("chmod control root");
        let metadata = fs::symlink_metadata(&root).expect("control metadata");
        let controller = Self {
            root,
            root_identity: root_identity(&metadata),
            state: Mutex::new(State::default()),
            ack_publish: Mutex::new(()),
        };
        (parent, controller)
    }
    /// Hold and FIFO-release every subsequently admitted message in tests.
    #[cfg(test)]
    pub(crate) fn drain_subsequent_messages_for_tests(&self) {
        let mut state = self.state.lock().expect("message control state poisoned");
        state.drain_next_rules = Some(Vec::new());
    }
    fn validate_root(&self) -> Result<(), ControlError> {
        let metadata = fs::symlink_metadata(&self.root).map_err(ControlError::Io)?;
        validate_private_root(&metadata)?;
        if root_identity(&metadata) != self.root_identity {
            return Err(ControlError::RootIdentityChanged);
        }
        Ok(())
    }
    /// Read and atomically apply one newer canonical command, if present.
    pub(crate) fn poll_command(&self) -> Result<(), ControlError> {
        self.validate_root()?;
        let path = self.root.join(CONTROL_FILE);
        let bytes = match read_stable_private_file(&path, MAX_COMMAND_BYTES) {
            Ok(bytes) => bytes,
            Err(ControlError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(ControlError::MissingCommand);
            }
            Err(error) => return Err(error),
        };
        self.validate_root()?;
        let digest = Hash::new(&bytes);
        {
            let state = self.state.lock().expect("message control state poisoned");
            if state.fatal {
                return Err(ControlError::ControllerFatal);
            }
            if state.last_seen_digest == Some(digest) {
                return Ok(());
            }
        }
        let parsed = parse_command(&bytes);
        let mut state = self.state.lock().expect("message control state poisoned");
        state.last_seen_digest = Some(digest);
        match parsed.and_then(|command| apply_command(&mut state, command, digest)) {
            Ok(()) => {}
            Err(error) => {
                state.rejected_commands = state.rejected_commands.saturating_add(1);
                state.last_error = Some(error.code().to_owned());
            }
        }
        drop(state);
        self.publish_ack()
    }
    /// Apply the current receiver-local rule to an authenticated inbound message.
    pub(crate) fn admit(
        &self,
        peer: Peer,
        authenticated_via: &PeerId,
        message: NetworkMessage,
        size_bytes: usize,
    ) -> Result<(Admission, Option<(Peer, NetworkMessage, usize)>), ControlError> {
        self.admit_with_reply_route(peer, authenticated_via, message, size_bytes, None)
            .map(|(admission, message)| {
                (
                    admission,
                    message.map(|(peer, message, size_bytes, reply_route)| {
                        debug_assert!(reply_route.is_none());
                        (peer, message, size_bytes)
                    }),
                )
            })
    }
    /// Apply one rule while retaining an exact local-only reply authority.
    pub(crate) fn admit_with_reply_route(
        &self,
        peer: Peer,
        authenticated_via: &PeerId,
        message: NetworkMessage,
        size_bytes: usize,
        reply_route: Option<R>,
    ) -> Result<(Admission, Option<(Peer, NetworkMessage, usize, Option<R>)>), ControlError> {
        self.admit_with_reply_route_and_ownership(
            peer,
            authenticated_via,
            message,
            size_bytes,
            reply_route,
            None,
        )
        .map(|(admission, message)| {
            (
                admission,
                message.map(|(peer, message, size_bytes, reply_route, ownership)| {
                    debug_assert!(ownership.is_none());
                    (peer, message, size_bytes, reply_route)
                }),
            )
        })
    }
    /// Apply one rule while atomically retaining reply authority and ownership.
    ///
    /// `ownership` is an opaque process-local token. A held occurrence stores it
    /// in the same map entry as its semantic payload and exact reply route, then
    /// returns it only from [`Self::next_release`]. This avoids a side-map race
    /// or a release-time reacquisition window.
    pub(crate) fn admit_with_reply_route_and_ownership(
        &self,
        peer: Peer,
        authenticated_via: &PeerId,
        message: NetworkMessage,
        size_bytes: usize,
        reply_route: Option<R>,
        ownership: Option<O>,
    ) -> Result<
        (
            Admission,
            Option<(Peer, NetworkMessage, usize, Option<R>, Option<O>)>,
        ),
        ControlError,
    > {
        let meta = match message_meta(&peer, authenticated_via, &message) {
            Ok(Some(meta)) => meta,
            Ok(None) => {
                return Ok((
                    Admission::Pass,
                    Some((peer, message, size_bytes, reply_route, ownership)),
                ));
            }
            Err(error) => {
                let mut state = self.state.lock().expect("message control state poisoned");
                state.fatal = true;
                state.last_error = Some(error.code().to_owned());
                drop(state);
                self.publish_ack()?;
                return Err(error);
            }
        };
        let mut state = self.state.lock().expect("message control state poisoned");
        if state.fatal {
            return Ok((Admission::Consumed, None));
        }
        let resolved_rule = match resolve_deferred_chunk_rules(&mut state, &meta) {
            Ok(resolved) => resolved,
            Err(error) => {
                state.fatal = true;
                state.last_error = Some(error.code().to_owned());
                drop(state);
                self.publish_ack()?;
                return Err(error);
            }
        };
        let draining = state.drain_next_rules.is_some();
        let action = if draining {
            Action::Hold
        } else if let Some(action) = state
            .rules
            .iter()
            .find(|rule| {
                rule_matches_with_proposal_evidence(rule, &meta, &state.proposal_round_evidence)
            })
            .map(|rule| rule.action)
        {
            action
        } else {
            drop(state);
            if resolved_rule {
                self.publish_ack()?;
            }
            return Ok((
                Admission::Pass,
                Some((peer, message, size_bytes, reply_route, ownership)),
            ));
        };
        match action {
            Action::Drop => {
                state.dropped = state.dropped.saturating_add(1);
                drop(state);
                self.publish_ack()?;
                return Ok((
                    Admission::Consumed,
                    Some((peer, message, size_bytes, reply_route, ownership)),
                ));
            }
            Action::Hold => {
                if !hold_capacity_available(&state, size_bytes) {
                    fail_hold_overflow(&mut state);
                    drop(state);
                    self.publish_ack()?;
                    return Err(ControlError::HoldQueueOverflow);
                } else {
                    let sequence = state.next_sequence;
                    let Some(next_sequence) = state.next_sequence.checked_add(1) else {
                        state.fatal = true;
                        state.last_error = Some("sequence_overflow".to_owned());
                        drop(state);
                        self.publish_ack()?;
                        return Err(ControlError::SequenceOverflow);
                    };
                    state.next_sequence = next_sequence;
                    let descriptor = HeldDescriptor {
                        sequence,
                        meta,
                        size_bytes,
                    };
                    state.held.insert(
                        sequence,
                        HeldEntry {
                            descriptor,
                            peer,
                            authenticated_via: authenticated_via.clone(),
                            message,
                            reply_route,
                            ownership,
                        },
                    );
                    state.held_bytes = state
                        .held_bytes
                        .checked_add(size_bytes)
                        .ok_or(ControlError::HeldBytesOverflow)?;
                    if draining {
                        state.release_pending.push_back(sequence);
                    }
                }
            }
        }
        drop(state);
        self.publish_ack()?;
        Ok((Admission::Held, None))
    }
    /// Take the next prevalidated release entry in exact ingress order.
    pub(crate) fn next_release(&self) -> Result<Option<HeldMessage<R, O>>, ControlError> {
        let mut state = self.state.lock().expect("message control state poisoned");
        if state.fatal || state.in_flight.is_some() {
            return Ok(None);
        }
        let Some(sequence) = state.release_pending.pop_front() else {
            return Ok(None);
        };
        let entry = state
            .held
            .remove(&sequence)
            .ok_or(ControlError::ReleaseEntryDisappeared)?;
        state.held_bytes = state
            .held_bytes
            .checked_sub(entry.descriptor.size_bytes)
            .ok_or(ControlError::HeldBytesUnderflow)?;
        state.in_flight = Some(sequence);
        state.in_flight_bytes = entry.descriptor.size_bytes;
        drop(state);
        self.publish_ack()?;
        Ok(Some(HeldMessage {
            sequence,
            peer: entry.peer,
            authenticated_via: entry.authenticated_via,
            message: entry.message,
            size_bytes: entry.descriptor.size_bytes,
            reply_route: entry.reply_route,
            ownership: entry.ownership,
        }))
    }
    /// Record exact completion of one release. Failure permanently closes the controller.
    pub(crate) fn complete_release(
        &self,
        sequence: u64,
        outcome: ReleaseOutcome,
    ) -> Result<(), ControlError> {
        let mut state = self.state.lock().expect("message control state poisoned");
        if state.in_flight != Some(sequence) {
            state.fatal = true;
            state.last_error = Some("release_completion_mismatch".to_owned());
            drop(state);
            self.publish_ack()?;
            return Err(ControlError::ReleaseCompletionMismatch);
        }
        state.in_flight = None;
        state.in_flight_bytes = 0;
        match outcome {
            ReleaseOutcome::Delivered => {
                state.delivered.push(sequence);
                finish_drain_if_empty(&mut state);
            }
            ReleaseOutcome::Retired => {
                state.retired.push(sequence);
                finish_drain_if_empty(&mut state);
            }
            ReleaseOutcome::Failed => {
                state.fatal = true;
                state.last_error = Some("downstream_delivery_failed".to_owned());
            }
        }
        drop(state);
        self.publish_ack()?;
        if outcome != ReleaseOutcome::Failed {
            Ok(())
        } else {
            Err(ControlError::DownstreamDeliveryFailed)
        }
    }
    fn publish_ack(&self) -> Result<(), ControlError> {
        let _publish = self
            .ack_publish
            .lock()
            .expect("message control ack publisher poisoned");
        self.validate_root()?;
        let bytes = {
            let state = self.state.lock().expect("message control state poisoned");
            canonical_json(&ack_value(&state)?)?
        };
        if bytes.len() > MAX_ACK_BYTES {
            return Err(ControlError::AckTooLarge);
        }
        write_atomic_private_file(&self.root, ACK_FILE, &bytes)?;
        self.validate_root()
    }
}
fn hold_capacity_available<R, O>(state: &State<R, O>, incoming_bytes: usize) -> bool {
    state
        .held
        .len()
        .checked_add(if state.in_flight.is_some() { 1 } else { 0 })
        .is_some_and(|count| count < state.queue_capacity)
        && state
            .held_bytes
            .checked_add(state.in_flight_bytes)
            .and_then(|bytes| bytes.checked_add(incoming_bytes))
            .is_some_and(|bytes| bytes <= MAX_HELD_BYTES)
}
fn fail_hold_overflow<R, O>(state: &mut State<R, O>) {
    state.overflowed = state.overflowed.saturating_add(1);
    state.fatal = true;
    state.last_error = Some("hold_queue_overflow".to_owned());
}
fn apply_command<R, O>(
    state: &mut State<R, O>,
    mut command: Command,
    digest: Hash,
) -> Result<(), ControlError> {
    if state.fatal {
        return Err(ControlError::ControllerFatal);
    }
    if command.revision <= state.revision {
        return Err(ControlError::StaleRevision);
    }
    if state.in_flight.is_some() || !state.release_pending.is_empty() {
        return Err(ControlError::ReleaseBusy);
    }
    if command.queue_capacity
        < state
            .held
            .len()
            .saturating_add(if state.in_flight.is_some() { 1 } else { 0 })
    {
        return Err(ControlError::QueueCapacityBelowHeld);
    }
    if command.drain && !command.release.is_empty() {
        return Err(ControlError::DrainWithExplicitRelease);
    }
    validate_release_sequences(&command.release, state.held.keys().copied())?;
    resolve_deferred_rules_from_evidence(&mut command.rules, &state.proposal_round_evidence)?;
    state.revision = command.revision;
    state.command_digest = Some(digest);
    state.queue_capacity = command.queue_capacity;
    state.delivered.clear();
    state.retired.clear();
    state.last_error = None;
    if command.drain {
        state.drain_fence = Some(command.revision);
        state.drain_next_rules = Some(command.rules);
        state.release_pending = state.held.keys().copied().collect();
        finish_drain_if_empty(state);
    } else {
        state.rules = command.rules;
        state.release_pending = command.release.into();
    }
    Ok(())
}
/// Activate post-drain rules at the same linearization point that observes all
/// retained and in-flight messages successfully delivered or retired.
fn finish_drain_if_empty<R, O>(state: &mut State<R, O>) {
    if state.held.is_empty()
        && state.release_pending.is_empty()
        && state.in_flight.is_none()
        && let Some(next_rules) = state.drain_next_rules.take()
    {
        state.rules = next_rules;
    }
}
fn validate_release_sequences(
    release: &[u64],
    held: impl IntoIterator<Item = u64>,
) -> Result<(), ControlError> {
    let held = held.into_iter().collect::<BTreeSet<_>>();
    let mut prior = None;
    for sequence in release {
        if prior.is_some_and(|previous| previous >= *sequence) {
            return Err(ControlError::ReleaseNotStrictlyIncreasing);
        }
        if !held.contains(sequence) {
            return Err(ControlError::UnknownReleaseSequence);
        }
        prior = Some(*sequence);
    }
    Ok(())
}
fn parse_command(bytes: &[u8]) -> Result<Command, ControlError> {
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err(ControlError::CommandTooLarge);
    }
    let value: Value = norito::json::from_slice(bytes).map_err(|_| ControlError::MalformedJson)?;
    if canonical_json(&value)?.as_slice() != bytes {
        return Err(ControlError::NonCanonicalJson);
    }
    let object = exact_object(
        &value,
        &[
            "drain",
            "queue_capacity",
            "release",
            "revision",
            "rules",
            "version",
        ],
    )?;
    if required_u64(object, "version")? != FORMAT_VERSION {
        return Err(ControlError::UnsupportedVersion);
    }
    let revision = required_u64(object, "revision")?;
    if revision == 0 {
        return Err(ControlError::InvalidField("revision"));
    }
    let queue_capacity_u64 = required_u64(object, "queue_capacity")?;
    let drain = object
        .get("drain")
        .and_then(Value::as_bool)
        .ok_or(ControlError::InvalidField("drain"))?;
    let queue_capacity = usize::try_from(queue_capacity_u64)
        .map_err(|_| ControlError::InvalidField("queue_capacity"))?;
    if queue_capacity == 0 || queue_capacity > MAX_HOLDS {
        return Err(ControlError::InvalidField("queue_capacity"));
    }
    let rules_value = object
        .get("rules")
        .and_then(Value::as_array)
        .ok_or(ControlError::InvalidField("rules"))?;
    if rules_value.len() > MAX_RULES {
        return Err(ControlError::TooManyRules);
    }
    let mut rules = Vec::with_capacity(rules_value.len());
    for value in rules_value {
        let rule = parse_rule(value)?;
        if rule.kind == MessageKind::PayloadChunk
            && rule.proposal_height.is_some()
            && rule.manifest_hash.is_some()
        {
            return Err(ControlError::InvalidField("manifest_hash"));
        }
        if rules.iter().any(|prior: &Rule| prior.overlaps(&rule)) {
            return Err(ControlError::AmbiguousRule);
        }
        rules.push(rule);
    }
    let release_value = object
        .get("release")
        .and_then(Value::as_array)
        .ok_or(ControlError::InvalidField("release"))?;
    if release_value.len() > MAX_RELEASES {
        return Err(ControlError::TooManyReleases);
    }
    let mut release = Vec::with_capacity(release_value.len());
    for value in release_value {
        release.push(
            value
                .as_u64()
                .ok_or(ControlError::InvalidField("release"))?,
        );
    }
    Ok(Command {
        revision,
        queue_capacity,
        rules,
        release,
        drain,
    })
}
fn parse_rule(value: &Value) -> Result<Rule, ControlError> {
    let object = exact_object(
        value,
        &[
            "action",
            "authenticated_via",
            "block_hash",
            "chunk_index",
            "height",
            "kind",
            "manifest_hash",
            "proposal_height",
            "proposal_view",
            "sender",
            "view",
        ],
    )?;
    let sender_literal = object
        .get("sender")
        .and_then(Value::as_str)
        .ok_or(ControlError::InvalidField("sender"))?;
    if sender_literal.is_empty() || sender_literal.len() > MAX_SENDER_BYTES {
        return Err(ControlError::FieldTooLarge("sender"));
    }
    let sender = sender_literal
        .parse::<PeerId>()
        .map_err(|_| ControlError::InvalidField("sender"))?;
    if sender.to_string() != sender_literal {
        return Err(ControlError::NonCanonicalField("sender"));
    }
    let authenticated_via_literal = object
        .get("authenticated_via")
        .and_then(Value::as_str)
        .ok_or(ControlError::InvalidField("authenticated_via"))?;
    if authenticated_via_literal.is_empty() || authenticated_via_literal.len() > MAX_SENDER_BYTES {
        return Err(ControlError::FieldTooLarge("authenticated_via"));
    }
    let authenticated_via = authenticated_via_literal
        .parse::<PeerId>()
        .map_err(|_| ControlError::InvalidField("authenticated_via"))?;
    if authenticated_via.to_string() != authenticated_via_literal {
        return Err(ControlError::NonCanonicalField("authenticated_via"));
    }
    let block_hash = match object.get("block_hash") {
        Some(Value::Null) => None,
        Some(value) => {
            let value = value
                .as_str()
                .ok_or(ControlError::InvalidField("block_hash"))?;
            if value.is_empty() || value.len() > MAX_HASH_BYTES {
                return Err(ControlError::FieldTooLarge("block_hash"));
            }
            let parsed = value
                .parse::<HashOf<BlockHeader>>()
                .map_err(|_| ControlError::InvalidField("block_hash"))?;
            if parsed.to_string() != value {
                return Err(ControlError::NonCanonicalField("block_hash"));
            }
            Some(parsed)
        }
        None => return Err(ControlError::InvalidField("block_hash")),
    };
    let kind = MessageKind::parse(
        object
            .get("kind")
            .ok_or(ControlError::InvalidField("kind"))?,
    )?;
    let height = optional_u64(object, "height")?;
    let view = optional_u64(object, "view")?;
    let manifest_hash = match object.get("manifest_hash") {
        Some(Value::Null) => None,
        Some(value) => {
            let value = value
                .as_str()
                .ok_or(ControlError::InvalidField("manifest_hash"))?;
            if value.is_empty() || value.len() > MAX_HASH_BYTES {
                return Err(ControlError::FieldTooLarge("manifest_hash"));
            }
            let parsed = value
                .parse::<HashOf<PayloadManifest>>()
                .map_err(|_| ControlError::InvalidField("manifest_hash"))?;
            if parsed.to_string() != value {
                return Err(ControlError::NonCanonicalField("manifest_hash"));
            }
            Some(parsed)
        }
        None => return Err(ControlError::InvalidField("manifest_hash")),
    };
    let chunk_index = optional_u64(object, "chunk_index")?
        .map(u32::try_from)
        .transpose()
        .map_err(|_| ControlError::InvalidField("chunk_index"))?;
    let proposal_height = optional_u64(object, "proposal_height")?;
    let proposal_view = optional_u64(object, "proposal_view")?;
    let proposal_binding_valid = match (proposal_height, proposal_view) {
        (None, None) => true,
        (Some(height), Some(_)) => height > 0,
        _ => false,
    };
    let action = Action::parse(
        object
            .get("action")
            .ok_or(ControlError::InvalidField("action"))?,
    )?;
    let valid_coordinates = match kind {
        MessageKind::PayloadChunk => {
            height.is_none()
                && view.is_none()
                && block_hash.is_none()
                && chunk_index.is_some()
                && proposal_binding_valid
                && (manifest_hash.is_some() || proposal_height.is_some())
                && (manifest_hash.is_some() || action == Action::Hold)
        }
        MessageKind::CommitCertificateRequest => {
            height.is_some_and(|height| height > 0)
                && view.is_none()
                && block_hash.is_none()
                && manifest_hash.is_none()
                && chunk_index.is_none()
                && proposal_height.is_none()
                && proposal_view.is_none()
        }
        _ => {
            height.is_some_and(|height| height > 0)
                && view.is_some()
                && manifest_hash.is_none()
                && chunk_index.is_none()
                && proposal_height.is_none()
                && proposal_view.is_none()
        }
    };
    if !valid_coordinates {
        return Err(ControlError::InvalidField("coordinates"));
    }
    Ok(Rule {
        sender,
        authenticated_via,
        kind,
        height,
        view,
        block_hash,
        manifest_hash,
        chunk_index,
        proposal_height,
        proposal_view,
        action,
    })
}
fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a Map, ControlError> {
    let object = value.as_object().ok_or(ControlError::ExpectedObject)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(ControlError::UnexpectedFields);
    }
    Ok(object)
}
fn required_u64(object: &Map, field: &'static str) -> Result<u64, ControlError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(ControlError::InvalidField(field))
}
fn optional_u64(object: &Map, field: &'static str) -> Result<Option<u64>, ControlError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(ControlError::InvalidField(field)),
        None => Err(ControlError::InvalidField(field)),
    }
}
fn canonical_json(value: &Value) -> Result<Vec<u8>, ControlError> {
    norito::json::to_json(value)
        .map(String::into_bytes)
        .map_err(|_| ControlError::JsonEncode)
}
fn ack_value<R, O>(state: &State<R, O>) -> Result<Value, ControlError> {
    let held = state
        .held
        .values()
        .map(|entry| descriptor_value(&entry.descriptor))
        .collect::<Result<Vec<_>, _>>()?;
    let rules = state.rules.iter().map(rule_value).collect::<Vec<_>>();
    let release_pending = state
        .release_pending
        .iter()
        .copied()
        .map(Value::from)
        .collect::<Vec<_>>();
    let delivered = state
        .delivered
        .iter()
        .copied()
        .map(Value::from)
        .collect::<Vec<_>>();
    let retired = state
        .retired
        .iter()
        .copied()
        .map(Value::from)
        .collect::<Vec<_>>();
    Ok(object_value([
        (
            "command_digest",
            state
                .command_digest
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        ("delivered", Value::Array(delivered)),
        ("dropped", Value::from(state.dropped)),
        (
            "drain_fence",
            state.drain_fence.map_or(Value::Null, Value::from),
        ),
        ("draining", Value::from(state.drain_next_rules.is_some())),
        ("fatal", Value::from(state.fatal)),
        ("held", Value::Array(held)),
        (
            "held_bytes",
            Value::from(u64::try_from(state.held_bytes).expect("held-byte bound fits u64")),
        ),
        (
            "in_flight",
            state.in_flight.map_or(Value::Null, Value::from),
        ),
        (
            "in_flight_bytes",
            Value::from(
                u64::try_from(state.in_flight_bytes).expect("in-flight byte bound fits u64"),
            ),
        ),
        (
            "last_error",
            state
                .last_error
                .as_ref()
                .map_or(Value::Null, |error| Value::from(error.clone())),
        ),
        ("overflowed", Value::from(state.overflowed)),
        (
            "queue_capacity",
            Value::from(u64::try_from(state.queue_capacity).expect("bounded capacity fits u64")),
        ),
        ("rejected_commands", Value::from(state.rejected_commands)),
        ("release_pending", Value::Array(release_pending)),
        ("retired", Value::Array(retired)),
        ("revision", Value::from(state.revision)),
        ("rules", Value::Array(rules)),
        ("version", Value::from(FORMAT_VERSION)),
    ]))
}
fn descriptor_value(descriptor: &HeldDescriptor) -> Result<Value, ControlError> {
    let subject = descriptor
        .meta
        .subject
        .as_ref()
        .map(norito::json::to_value)
        .transpose()
        .map_err(|_| ControlError::JsonEncode)?
        .unwrap_or(Value::Null);
    let execution_commitment = descriptor
        .meta
        .execution_commitment
        .as_ref()
        .map(norito::json::to_value)
        .transpose()
        .map_err(|_| ControlError::JsonEncode)?
        .unwrap_or(Value::Null);
    let certificate_signers = descriptor
        .meta
        .certificate_signers
        .iter()
        .copied()
        .map(|signer| Value::from(u64::from(signer)))
        .collect::<Vec<_>>();
    Ok(object_value([
        (
            "authenticated_via",
            Value::from(descriptor.meta.authenticated_via.to_string()),
        ),
        (
            "block_hash",
            descriptor
                .meta
                .block_hash
                .as_ref()
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        ("certificate_signers", Value::Array(certificate_signers)),
        (
            "chunk_index",
            descriptor.meta.chunk_index.map_or(Value::Null, Value::from),
        ),
        (
            "cited_responder",
            descriptor
                .meta
                .cited_responder
                .as_ref()
                .map_or(Value::Null, |responder| Value::from(responder.to_string())),
        ),
        (
            "envelope_digest",
            Value::from(descriptor.meta.envelope_digest.to_string()),
        ),
        ("execution_commitment", execution_commitment),
        (
            "height",
            descriptor.meta.height.map_or(Value::Null, Value::from),
        ),
        ("kind", Value::from(descriptor.meta.kind.as_str())),
        (
            "manifest_hash",
            descriptor
                .meta
                .manifest_hash
                .as_ref()
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        ("sender", Value::from(descriptor.meta.sender.to_string())),
        ("sequence", Value::from(descriptor.sequence)),
        (
            "signer",
            descriptor
                .meta
                .signer
                .map_or(Value::Null, |signer| Value::from(u64::from(signer))),
        ),
        (
            "size_bytes",
            Value::from(u64::try_from(descriptor.size_bytes).unwrap_or(u64::MAX)),
        ),
        ("subject", subject),
        (
            "view",
            descriptor.meta.view.map_or(Value::Null, Value::from),
        ),
    ]))
}
fn rule_value(rule: &Rule) -> Value {
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
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        (
            "chunk_index",
            rule.chunk_index.map_or(Value::Null, Value::from),
        ),
        ("height", rule.height.map_or(Value::Null, Value::from)),
        ("kind", Value::from(rule.kind.as_str())),
        (
            "manifest_hash",
            rule.manifest_hash
                .as_ref()
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        (
            "proposal_height",
            rule.proposal_height.map_or(Value::Null, Value::from),
        ),
        (
            "proposal_view",
            rule.proposal_view.map_or(Value::Null, Value::from),
        ),
        ("sender", Value::from(rule.sender.to_string())),
        ("view", rule.view.map_or(Value::Null, Value::from)),
    ])
}
fn object_value<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in entries {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}
fn message_meta(
    peer: &Peer,
    authenticated_via: &PeerId,
    message: &NetworkMessage,
) -> Result<Option<MessageMeta>, ControlError> {
    let NetworkMessage::SumeragiBlock(block) = message else {
        return Ok(None);
    };
    let iroha_core::sumeragi::message::BlockMessage::V2(message) = block.as_ref().as_ref() else {
        return Ok(None);
    };
    let sender = peer.id().clone();
    let envelope_digest = Hash::new(message.encode());
    let (kind, round, subject, execution_commitment, signer, certificate_signers) =
        match &message.payload {
            ConsensusMessageV2Payload::Proposal(value) => (
                MessageKind::Proposal,
                Some(value.round),
                Some(value.subject),
                None,
                Some(value.proposer),
                Vec::new(),
            ),
            ConsensusMessageV2Payload::Vote(value) => (
                match value.phase {
                    GlobalPhase::Prepare => MessageKind::PrepareVote,
                    GlobalPhase::Commit => MessageKind::CommitVote,
                },
                Some(value.round),
                Some(value.subject),
                Some(value.execution_commitment),
                Some(value.signer),
                Vec::new(),
            ),
            ConsensusMessageV2Payload::QuorumCertificate(value) => (
                match value.phase {
                    GlobalPhase::Prepare => MessageKind::PrepareCertificate,
                    GlobalPhase::Commit => MessageKind::CommitCertificate,
                },
                Some(value.round),
                Some(value.subject),
                Some(value.execution_commitment),
                None,
                value.signers.clone(),
            ),
            ConsensusMessageV2Payload::TimeoutVote(value) => (
                MessageKind::TimeoutVote,
                Some(value.round),
                value.highest_prepare_qc.as_ref().map(|qc| qc.subject),
                value
                    .highest_prepare_qc
                    .as_ref()
                    .map(|qc| qc.execution_commitment),
                Some(value.signer),
                Vec::new(),
            ),
            ConsensusMessageV2Payload::TimeoutCertificate(value) => {
                let highest = value.highest_prepare_qc();
                let mut signers = value
                    .groups
                    .iter()
                    .flat_map(|group| group.signers.iter().copied())
                    .collect::<Vec<_>>();
                signers.sort_unstable();
                (
                    MessageKind::TimeoutCertificate,
                    Some(value.round),
                    highest.map(|qc| qc.subject),
                    highest.map(|qc| qc.execution_commitment),
                    None,
                    signers,
                )
            }
            ConsensusMessageV2Payload::PayloadChunk(value) => (
                MessageKind::PayloadChunk,
                None,
                None,
                None,
                Some(value.sender),
                Vec::new(),
            ),
            ConsensusMessageV2Payload::CertifiedBodyRequest(value) => (
                MessageKind::CertifiedBodyRequest,
                Some(value.round),
                Some(value.subject),
                Some(value.certificate.execution_commitment),
                None,
                value.certificate.signers.clone(),
            ),
            ConsensusMessageV2Payload::CertifiedBodyResponse(value) => (
                MessageKind::CertifiedBodyResponse,
                Some(value.manifest.round),
                Some(value.manifest.subject),
                None,
                None,
                Vec::new(),
            ),
            ConsensusMessageV2Payload::CommitCertificateRequest(value) => {
                let meta = MessageMeta {
                    sender,
                    authenticated_via: authenticated_via.clone(),
                    kind: MessageKind::CommitCertificateRequest,
                    height: Some(value.height),
                    view: None,
                    block_hash: None,
                    manifest_hash: None,
                    chunk_index: None,
                    subject: None,
                    execution_commitment: None,
                    signer: None,
                    cited_responder: None,
                    certificate_signers: Vec::new(),
                    envelope_digest,
                };
                validate_message_meta(&meta)?;
                return Ok(Some(meta));
            }
            ConsensusMessageV2Payload::CommitCertificateResponse(value) => (
                MessageKind::CommitCertificateResponse,
                Some(value.certificate.round),
                Some(value.certificate.subject),
                Some(value.certificate.execution_commitment),
                None,
                value.certificate.signers.clone(),
            ),
            ConsensusMessageV2Payload::GlobalBeaconPartialSignature(value) => (
                MessageKind::GlobalBeaconPartialSignature,
                Some(value.round),
                None,
                None,
                value.partial.signer_index.checked_sub(1).map(u32::from),
                Vec::new(),
            ),
        };
    let cited_responder = match &message.payload {
        ConsensusMessageV2Payload::CertifiedBodyResponse(value) => Some(value.responder.clone()),
        _ => None,
    };
    let (manifest_hash, chunk_index) = match &message.payload {
        ConsensusMessageV2Payload::Proposal(value) => (Some(HashOf::new(&value.manifest)), None),
        ConsensusMessageV2Payload::PayloadChunk(value) => {
            (Some(value.manifest_hash), Some(value.index))
        }
        ConsensusMessageV2Payload::CertifiedBodyResponse(value) => {
            (Some(HashOf::new(&value.manifest)), None)
        }
        _ => (None, None),
    };
    let meta = MessageMeta {
        sender,
        authenticated_via: authenticated_via.clone(),
        kind,
        height: round.map(|round| round.height),
        view: round.map(|round| round.view),
        block_hash: subject.map(|subject| subject.block_hash),
        manifest_hash,
        chunk_index,
        subject,
        execution_commitment,
        signer,
        cited_responder,
        certificate_signers,
        envelope_digest,
    };
    validate_message_meta(&meta)?;
    Ok(Some(meta))
}
/// Validate the exact JSON descriptor contract before a controlled message can
/// enter the hold queue.
///
/// This is intentionally structural rather than a substitute for ordinary
/// consensus authentication. The controller runs before reducer ingress, but
/// every descriptor it publishes must still be parseable by the independent
/// test-network client. Invalid traffic therefore fails the controller closed
/// without poisoning its acknowledgement with an unrepresentable entry.
fn validate_message_meta(meta: &MessageMeta) -> Result<(), ControlError> {
    if meta.height == Some(0)
        || meta
            .execution_commitment
            .as_ref()
            .is_some_and(|commitment| commitment.validate().is_err())
        || meta.certificate_signers.len() > MAX_VALIDATORS_PER_HEIGHT
        || meta
            .certificate_signers
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || meta.block_hash != meta.subject.map(|subject| subject.block_hash)
        || meta.execution_commitment.is_some() && meta.subject.is_none()
        || meta.kind == MessageKind::PayloadChunk
            && (meta.manifest_hash.is_some() != meta.chunk_index.is_some())
    {
        return Err(ControlError::InvalidMessageDescriptor);
    }
    let has_subject_and_execution = meta.subject.is_some() && meta.execution_commitment.is_some();
    let has_no_subject_or_execution = meta.subject.is_none() && meta.execution_commitment.is_none();
    let has_single_signer = meta.signer.is_some();
    let has_cited_responder = meta.cited_responder.is_some();
    let has_certificate_signers = !meta.certificate_signers.is_empty();
    let has_manifest_hash = meta.manifest_hash.is_some();
    let has_chunk_index = meta.chunk_index.is_some();
    if (meta.kind == MessageKind::CertifiedBodyResponse) != has_cited_responder {
        return Err(ControlError::InvalidMessageDescriptor);
    }
    let has_round = meta.height.is_some() && meta.view.is_some();
    let valid = match meta.kind {
        MessageKind::Proposal => {
            has_round
                && meta.subject.is_some()
                && meta.execution_commitment.is_none()
                && has_single_signer
                && !has_certificate_signers
                && has_manifest_hash
                && !has_chunk_index
        }
        MessageKind::PrepareVote | MessageKind::CommitVote => {
            has_round && has_subject_and_execution && has_single_signer && !has_certificate_signers
        }
        MessageKind::PrepareCertificate
        | MessageKind::CommitCertificate
        | MessageKind::CertifiedBodyRequest
        | MessageKind::CommitCertificateResponse => {
            has_round && has_subject_and_execution && !has_single_signer && has_certificate_signers
        }
        MessageKind::TimeoutVote => {
            has_round
                && (has_subject_and_execution || has_no_subject_or_execution)
                && has_single_signer
                && !has_certificate_signers
        }
        MessageKind::TimeoutCertificate => {
            has_round
                && (has_subject_and_execution || has_no_subject_or_execution)
                && !has_single_signer
                && has_certificate_signers
        }
        MessageKind::PayloadChunk => {
            meta.height.is_none()
                && meta.view.is_none()
                && has_no_subject_or_execution
                && has_single_signer
                && !has_certificate_signers
                && has_manifest_hash
                && has_chunk_index
        }
        MessageKind::GlobalBeaconPartialSignature => {
            has_round
                && has_no_subject_or_execution
                && has_single_signer
                && !has_certificate_signers
                && !has_manifest_hash
                && !has_chunk_index
        }
        MessageKind::CertifiedBodyResponse => {
            has_round
                && meta.subject.is_some()
                && meta.execution_commitment.is_none()
                && !has_single_signer
                && !has_certificate_signers
                && has_manifest_hash
                && !has_chunk_index
        }
        MessageKind::CommitCertificateRequest => {
            meta.height.is_some()
                && meta.view.is_none()
                && has_no_subject_or_execution
                && !has_single_signer
                && !has_certificate_signers
        }
    };
    if !matches!(
        meta.kind,
        MessageKind::Proposal | MessageKind::PayloadChunk | MessageKind::CertifiedBodyResponse
    ) && (has_manifest_hash || has_chunk_index)
    {
        return Err(ControlError::InvalidMessageDescriptor);
    }
    if valid {
        Ok(())
    } else {
        Err(ControlError::InvalidMessageDescriptor)
    }
}
fn read_stable_private_file(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ControlError> {
    retry_file_identity_change_once(|| read_stable_private_file_after_open(path, max_bytes, || {}))
}
fn retry_file_identity_change_once(
    mut read: impl FnMut() -> Result<Vec<u8>, ControlError>,
) -> Result<Vec<u8>, ControlError> {
    match read() {
        Err(ControlError::FileIdentityChanged) => read(),
        result => result,
    }
}
fn read_stable_private_file_after_open(
    path: &Path,
    max_bytes: usize,
    after_open_before_metadata: impl FnOnce(),
) -> Result<Vec<u8>, ControlError> {
    let named_before = fs::symlink_metadata(path).map_err(ControlError::Io)?;
    validate_private_file(&named_before)?;
    if usize::try_from(named_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(ControlError::CommandTooLarge);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    let mut file = options.open(path).map_err(ControlError::Io)?;
    after_open_before_metadata();
    let opened_before = file.metadata().map_err(ControlError::Io)?;
    validate_opened_private_file(&opened_before)?;
    if !same_file(&named_before, &opened_before) {
        return Err(ControlError::FileIdentityChanged);
    }
    if usize::try_from(opened_before.len())
        .ok()
        .is_none_or(|length| length > max_bytes)
    {
        return Err(ControlError::CommandTooLarge);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_before.len())
            .unwrap_or(max_bytes)
            .min(max_bytes),
    );
    Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes).expect("command bound fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(ControlError::Io)?;
    if bytes.len() > max_bytes {
        return Err(ControlError::CommandTooLarge);
    }
    file.seek(SeekFrom::Start(0)).map_err(ControlError::Io)?;
    let mut confirmation = Vec::with_capacity(bytes.len());
    Read::by_ref(&mut file)
        .take(u64::try_from(max_bytes).expect("command bound fits u64") + 1)
        .read_to_end(&mut confirmation)
        .map_err(ControlError::Io)?;
    if confirmation != bytes {
        return Err(ControlError::FileIdentityChanged);
    }
    let opened_after = file.metadata().map_err(ControlError::Io)?;
    let named_after = fs::symlink_metadata(path).map_err(ControlError::Io)?;
    validate_private_file(&named_after)?;
    if !same_file(&opened_before, &opened_after)
        || !same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || opened_before.modified().ok() != opened_after.modified().ok()
    {
        return Err(ControlError::FileIdentityChanged);
    }
    validate_opened_private_file(&opened_after)?;
    Ok(bytes)
}
fn write_atomic_private_file(root: &Path, name: &str, bytes: &[u8]) -> Result<(), ControlError> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(".{name}.{}.{}.tmp", std::process::id(), sequence);
    let temp_path = root.join(temp_name);
    let final_path = root.join(name);
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => validate_private_file(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(ControlError::Io(error)),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp_path).map_err(ControlError::Io)?;
    let result = (|| {
        file.write_all(bytes).map_err(ControlError::Io)?;
        file.sync_all().map_err(ControlError::Io)?;
        let written = file.metadata().map_err(ControlError::Io)?;
        validate_private_file(&written)?;
        if written.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX) {
            return Err(ControlError::FileIdentityChanged);
        }
        fs::rename(&temp_path, &final_path).map_err(ControlError::Io)?;
        let installed = fs::symlink_metadata(&final_path).map_err(ControlError::Io)?;
        validate_private_file(&installed)?;
        if !same_file(&written, &installed) || installed.len() != written.len() {
            return Err(ControlError::FileIdentityChanged);
        }
        File::open(root)
            .and_then(|directory| directory.sync_all())
            .map_err(ControlError::Io)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}
fn validate_private_root(metadata: &fs::Metadata) -> Result<(), ControlError> {
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(ControlError::UnsafeRoot);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o777 != 0o700
        {
            return Err(ControlError::UnsafePermissions);
        }
    }
    Ok(())
}
fn validate_private_file(metadata: &fs::Metadata) -> Result<(), ControlError> {
    validate_private_file_without_link_count(metadata)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(ControlError::UnsafePermissions);
        }
    }
    Ok(())
}
fn validate_opened_private_file(metadata: &fs::Metadata) -> Result<(), ControlError> {
    validate_private_file_without_link_count(metadata)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match metadata.nlink() {
            0 => return Err(ControlError::FileIdentityChanged),
            1 => {}
            _ => return Err(ControlError::UnsafePermissions),
        }
    }
    Ok(())
}
fn validate_private_file_without_link_count(metadata: &fs::Metadata) -> Result<(), ControlError> {
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ControlError::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o777 != 0o600
        {
            return Err(ControlError::UnsafePermissions);
        }
    }
    Ok(())
}
fn root_identity(metadata: &fs::Metadata) -> RootIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        RootIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        RootIdentity {}
    }
}
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len()
            && left.modified().ok() == right.modified().ok()
            && left.created().ok() == right.created().ok()
    }
}
#[derive(Debug)]
pub(crate) enum ControlError {
    Io(std::io::Error),
    UnsafeRoot,
    UnsafeFile,
    UnsafePermissions,
    RootIdentityChanged,
    FileIdentityChanged,
    MissingCommand,
    CommandTooLarge,
    AckTooLarge,
    MalformedJson,
    NonCanonicalJson,
    NonCanonicalField(&'static str),
    JsonEncode,
    ExpectedObject,
    UnexpectedFields,
    InvalidField(&'static str),
    FieldTooLarge(&'static str),
    UnsupportedVersion,
    TooManyRules,
    TooManyReleases,
    AmbiguousRule,
    StaleRevision,
    ReleaseBusy,
    QueueCapacityBelowHeld,
    DrainWithExplicitRelease,
    ReleaseNotStrictlyIncreasing,
    UnknownReleaseSequence,
    ReleaseEntryDisappeared,
    ReleaseCompletionMismatch,
    DownstreamDeliveryFailed,
    ControllerFatal,
    ControllerUninitialized,
    SequenceOverflow,
    HeldBytesOverflow,
    HeldBytesUnderflow,
    HoldQueueOverflow,
    InvalidMessageDescriptor,
}
impl ControlError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io_error",
            Self::UnsafeRoot => "unsafe_root",
            Self::UnsafeFile => "unsafe_file",
            Self::UnsafePermissions => "unsafe_permissions",
            Self::RootIdentityChanged => "root_identity_changed",
            Self::FileIdentityChanged => "file_identity_changed",
            Self::MissingCommand => "missing_command",
            Self::CommandTooLarge => "command_too_large",
            Self::AckTooLarge => "ack_too_large",
            Self::MalformedJson => "malformed_json",
            Self::NonCanonicalJson => "noncanonical_json",
            Self::NonCanonicalField(_) => "noncanonical_field",
            Self::JsonEncode => "json_encode",
            Self::ExpectedObject => "expected_object",
            Self::UnexpectedFields => "unexpected_fields",
            Self::InvalidField(_) => "invalid_field",
            Self::FieldTooLarge(_) => "field_too_large",
            Self::UnsupportedVersion => "unsupported_version",
            Self::TooManyRules => "too_many_rules",
            Self::TooManyReleases => "too_many_releases",
            Self::AmbiguousRule => "ambiguous_rule",
            Self::StaleRevision => "stale_revision",
            Self::ReleaseBusy => "release_busy",
            Self::QueueCapacityBelowHeld => "queue_capacity_below_held",
            Self::DrainWithExplicitRelease => "drain_with_explicit_release",
            Self::ReleaseNotStrictlyIncreasing => "release_not_strictly_increasing",
            Self::UnknownReleaseSequence => "unknown_release_sequence",
            Self::ReleaseEntryDisappeared => "release_entry_disappeared",
            Self::ReleaseCompletionMismatch => "release_completion_mismatch",
            Self::DownstreamDeliveryFailed => "downstream_delivery_failed",
            Self::ControllerFatal => "controller_fatal",
            Self::ControllerUninitialized => "controller_uninitialized",
            Self::SequenceOverflow => "sequence_overflow",
            Self::HeldBytesOverflow => "held_bytes_overflow",
            Self::HeldBytesUnderflow => "held_bytes_underflow",
            Self::HoldQueueOverflow => "hold_queue_overflow",
            Self::InvalidMessageDescriptor => "invalid_message_descriptor",
        }
    }
}
impl std::fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{}: {error}", self.code()),
            Self::InvalidField(field)
            | Self::FieldTooLarge(field)
            | Self::NonCanonicalField(field) => {
                write!(formatter, "{}: {field}", self.code())
            }
            _ => formatter.write_str(self.code()),
        }
    }
}
impl std::error::Error for ControlError {}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::sumeragi::message::{BlockMessage, BlockMessageWire};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::block::consensus_v2::{
        ConsensusMessageV2, PayloadChunk, PayloadManifest,
    };
    use std::sync::Arc;
    use tempfile::tempdir;
    fn peer(marker: u8) -> PeerId {
        let key = KeyPair::try_from_seed(vec![marker; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        PeerId::new(key.public_key().clone())
    }
    fn hash(marker: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([marker; Hash::LENGTH]))
    }
    fn manifest_hash(marker: u8) -> HashOf<PayloadManifest> {
        HashOf::from_untyped_unchecked(Hash::prehashed([marker; Hash::LENGTH]))
    }
    fn subject(marker: u8) -> BlockSubject {
        BlockSubject {
            parent_block_hash: Some(hash(marker.wrapping_add(1))),
            block_hash: hash(marker),
            payload_hash: Hash::new([marker, 0xA5]),
        }
    }
    fn execution_commitment(marker: u8) -> ExecutionCommitment {
        ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new([marker, 1]),
            Hash::new([marker, 2]),
            Hash::new([marker, 3]),
            1,
            Hash::new([marker, 4]),
        )
    }
    fn valid_meta(kind: MessageKind) -> MessageMeta {
        let sender = peer(42);
        let subject = subject(7);
        let (
            height,
            view,
            subject,
            execution_commitment,
            signer,
            cited_responder,
            certificate_signers,
        ) = match kind {
            MessageKind::Proposal => (
                Some(9),
                Some(2),
                Some(subject),
                None,
                Some(0),
                None,
                Vec::new(),
            ),
            MessageKind::PrepareVote | MessageKind::CommitVote => (
                Some(9),
                Some(2),
                Some(subject),
                Some(execution_commitment(7)),
                Some(0),
                None,
                Vec::new(),
            ),
            MessageKind::PrepareCertificate
            | MessageKind::CommitCertificate
            | MessageKind::CertifiedBodyRequest
            | MessageKind::CommitCertificateResponse => (
                Some(9),
                Some(2),
                Some(subject),
                Some(execution_commitment(7)),
                None,
                None,
                vec![0, 1, 2],
            ),
            MessageKind::TimeoutVote => (Some(9), Some(2), None, None, Some(0), None, Vec::new()),
            MessageKind::TimeoutCertificate => {
                (Some(9), Some(2), None, None, None, None, vec![0, 1, 2])
            }
            MessageKind::PayloadChunk => (None, None, None, None, Some(0), None, Vec::new()),
            MessageKind::GlobalBeaconPartialSignature => {
                (Some(9), Some(2), None, None, Some(0), None, Vec::new())
            }
            MessageKind::CertifiedBodyResponse => (
                Some(9),
                Some(2),
                Some(subject),
                None,
                None,
                Some(sender.clone()),
                Vec::new(),
            ),
            MessageKind::CommitCertificateRequest => {
                (Some(9), None, None, None, None, None, Vec::new())
            }
        };
        let (manifest_hash, chunk_index) = match kind {
            MessageKind::Proposal | MessageKind::CertifiedBodyResponse => {
                (Some(manifest_hash(0x33)), None)
            }
            MessageKind::PayloadChunk => (Some(manifest_hash(0x33)), Some(7)),
            _ => (None, None),
        };
        MessageMeta {
            sender: sender.clone(),
            authenticated_via: sender,
            kind,
            height,
            view,
            block_hash: subject.map(|subject| subject.block_hash),
            manifest_hash,
            chunk_index,
            subject,
            execution_commitment,
            signer,
            cited_responder,
            certificate_signers,
            envelope_digest: Hash::new([kind as u8, 0x5A]),
        }
    }
    fn transport_peer(marker: u8) -> Peer {
        let key = KeyPair::try_from_seed(vec![marker; 32], Algorithm::Ed25519)
            .expect("deterministic transport key");
        Peer::new(
            "127.0.0.1:0".parse().expect("test address"),
            key.public_key().clone(),
        )
    }
    fn chunk_message(marker: u8) -> NetworkMessage {
        NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(BlockMessage::V2(
            ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: HashOf::<PayloadManifest>::from_untyped_unchecked(Hash::new(&[
                    marker,
                ])),
                index: u32::from(marker),
                bytes: vec![marker],
                sender: 0,
                signature: vec![marker; 64],
            })),
        ))))
    }
    fn test_controller() -> (tempfile::TempDir, Controller) {
        Controller::for_tests()
    }
    fn rule(sender: PeerId, kind: MessageKind, height: u64, view: u64) -> Rule {
        Rule {
            authenticated_via: sender.clone(),
            sender,
            kind,
            height: Some(height),
            view: Some(view),
            block_hash: Some(hash(1)),
            manifest_hash: None,
            chunk_index: None,
            proposal_height: None,
            proposal_view: None,
            action: Action::Hold,
        }
    }
    fn proposal_meta_at(height: u64, view: u64, manifest_marker: u8) -> MessageMeta {
        MessageMeta {
            height: Some(height),
            view: Some(view),
            manifest_hash: Some(manifest_hash(manifest_marker)),
            ..valid_meta(MessageKind::Proposal)
        }
    }
    fn chunk_meta_for(proposal: &MessageMeta, index: u32) -> MessageMeta {
        MessageMeta {
            sender: proposal.sender.clone(),
            authenticated_via: proposal.authenticated_via.clone(),
            manifest_hash: proposal.manifest_hash,
            chunk_index: Some(index),
            ..valid_meta(MessageKind::PayloadChunk)
        }
    }
    fn deferred_chunk_rule_for(
        proposal_route: &MessageMeta,
        height: u64,
        view: u64,
        index: u32,
    ) -> Rule {
        Rule {
            sender: proposal_route.sender.clone(),
            authenticated_via: proposal_route.authenticated_via.clone(),
            kind: MessageKind::PayloadChunk,
            height: None,
            view: None,
            block_hash: None,
            manifest_hash: None,
            chunk_index: Some(index),
            proposal_height: Some(height),
            proposal_view: Some(view),
            action: Action::Hold,
        }
    }
    fn retain_chunk(state: &mut State<NetworkReplyRoute>, sequence: u64, chunk: MessageMeta) {
        let authenticated_via = chunk.authenticated_via.clone();
        state.held.insert(
            sequence,
            HeldEntry {
                descriptor: HeldDescriptor {
                    sequence,
                    meta: chunk,
                    size_bytes: 1,
                },
                peer: transport_peer(42),
                authenticated_via,
                message: chunk_message(7),
                reply_route: None,
                ownership: None,
            },
        );
        state.held_bytes += 1;
    }
    #[test]
    fn matcher_is_exact_across_every_rule_dimension() {
        let peer_a = peer(1);
        let rule = rule(peer_a.clone(), MessageKind::PrepareVote, 9, 3);
        let exact = MessageMeta {
            sender: peer_a.clone(),
            authenticated_via: peer_a,
            kind: MessageKind::PrepareVote,
            height: Some(9),
            view: Some(3),
            block_hash: Some(hash(1)),
            manifest_hash: None,
            chunk_index: None,
            subject: None,
            execution_commitment: None,
            signer: Some(0),
            cited_responder: None,
            certificate_signers: Vec::new(),
            envelope_digest: Hash::new(b"exact-envelope"),
        };
        assert!(rule.matches(&exact));
        for changed in [
            MessageMeta {
                sender: peer(2),
                ..exact.clone()
            },
            MessageMeta {
                authenticated_via: peer(2),
                ..exact.clone()
            },
            MessageMeta {
                kind: MessageKind::CommitVote,
                ..exact.clone()
            },
            MessageMeta {
                height: Some(10),
                ..exact.clone()
            },
            MessageMeta {
                view: Some(4),
                ..exact.clone()
            },
            MessageMeta {
                block_hash: Some(hash(2)),
                ..exact.clone()
            },
        ] {
            assert!(!rule.matches(&changed));
        }
    }
    #[test]
    fn payload_chunk_matcher_binds_authenticated_manifest_and_index_without_round() {
        let sender = peer(5);
        let chunk_rule = Rule {
            sender: sender.clone(),
            authenticated_via: sender.clone(),
            kind: MessageKind::PayloadChunk,
            height: None,
            view: None,
            block_hash: None,
            manifest_hash: Some(manifest_hash(0x44)),
            chunk_index: Some(7),
            proposal_height: None,
            proposal_view: None,
            action: Action::Hold,
        };
        let exact = MessageMeta {
            sender: sender.clone(),
            authenticated_via: sender,
            kind: MessageKind::PayloadChunk,
            height: None,
            view: None,
            block_hash: None,
            manifest_hash: Some(manifest_hash(0x44)),
            chunk_index: Some(7),
            subject: None,
            execution_commitment: None,
            signer: Some(0),
            cited_responder: None,
            certificate_signers: Vec::new(),
            envelope_digest: Hash::new(b"exact-chunk-envelope"),
        };
        assert!(chunk_rule.matches(&exact));
        for changed in [
            MessageMeta {
                authenticated_via: peer(6),
                ..exact.clone()
            },
            MessageMeta {
                manifest_hash: Some(manifest_hash(0x45)),
                ..exact.clone()
            },
            MessageMeta {
                chunk_index: Some(8),
                ..exact.clone()
            },
            MessageMeta {
                height: Some(9),
                view: Some(0),
                ..exact.clone()
            },
        ] {
            assert!(!chunk_rule.matches(&changed));
        }

        let deferred = Rule {
            manifest_hash: None,
            proposal_height: Some(9),
            proposal_view: Some(2),
            ..chunk_rule
        };
        assert!(
            deferred.matches(&MessageMeta {
                manifest_hash: Some(manifest_hash(0x45)),
                ..exact
            }),
            "an unresolved Hold rule retains an authenticated candidate instead of racing it"
        );
    }
    #[test]
    fn authenticated_proposal_atomically_resolves_deferred_chunk_rule() {
        let proposal = valid_meta(MessageKind::Proposal);
        let mut state = State::<NetworkReplyRoute>::default();
        state.rules.push(Rule {
            sender: proposal.sender.clone(),
            authenticated_via: proposal.authenticated_via.clone(),
            kind: MessageKind::PayloadChunk,
            height: None,
            view: None,
            block_hash: None,
            manifest_hash: None,
            chunk_index: Some(7),
            proposal_height: proposal.height,
            proposal_view: proposal.view,
            action: Action::Hold,
        });
        for (sequence, chunk) in [
            (1, valid_meta(MessageKind::PayloadChunk)),
            (
                2,
                MessageMeta {
                    manifest_hash: Some(manifest_hash(0x34)),
                    ..valid_meta(MessageKind::PayloadChunk)
                },
            ),
        ] {
            state.held.insert(
                sequence,
                HeldEntry {
                    descriptor: HeldDescriptor {
                        sequence,
                        meta: chunk,
                        size_bytes: 1,
                    },
                    peer: transport_peer(42),
                    authenticated_via: proposal.authenticated_via.clone(),
                    message: chunk_message(7),
                    reply_route: None,
                    ownership: None,
                },
            );
            state.held_bytes += 1;
        }
        assert!(resolve_deferred_chunk_rules(&mut state, &proposal).expect("resolve rule"));
        assert_eq!(state.rules[0].manifest_hash, proposal.manifest_hash);
        assert!(state.rules[0].matches(&valid_meta(MessageKind::PayloadChunk)));
        assert_eq!(
            state.release_pending,
            VecDeque::from([2]),
            "a provisionally retained chunk from another manifest is released"
        );
        assert!(!resolve_deferred_chunk_rules(&mut state, &proposal).expect("idempotent resolve"));
    }
    #[test]
    fn prior_round_proposal_evidence_bypasses_unresolved_future_chunk_rule() {
        let prior_proposal = proposal_meta_at(9, 0, 0x31);
        let mut state = State::<NetworkReplyRoute>::default();
        state
            .rules
            .push(deferred_chunk_rule_for(&prior_proposal, 10, 0, 7));

        assert!(
            !resolve_deferred_chunk_rules(&mut state, &prior_proposal)
                .expect("record prior-round evidence"),
            "a non-target Proposal must not resolve the future-round rule"
        );
        assert_eq!(state.rules[0].manifest_hash, None);
        let prior_chunk = chunk_meta_for(&prior_proposal, 7);
        assert!(state.rules[0].matches(&prior_chunk));
        assert!(
            !rule_matches_with_proposal_evidence(
                &state.rules[0],
                &prior_chunk,
                &state.proposal_round_evidence,
            ),
            "the same authenticated route proves this compact chunk belongs to the prior round"
        );
        let other_via = peer(43);
        let other_route_chunk = MessageMeta {
            authenticated_via: other_via,
            ..prior_chunk
        };
        let other_route_rule = deferred_chunk_rule_for(&other_route_chunk, 10, 0, 7);
        assert!(rule_matches_with_proposal_evidence(
            &other_route_rule,
            &other_route_chunk,
            &state.proposal_round_evidence,
        ));
    }
    #[test]
    fn prior_round_proposal_releases_provisional_chunk_without_resolving_future_rule() {
        let prior_proposal = proposal_meta_at(9, 0, 0x32);
        let mut state = State::<NetworkReplyRoute>::default();
        state
            .rules
            .push(deferred_chunk_rule_for(&prior_proposal, 10, 0, 7));
        retain_chunk(&mut state, 1, chunk_meta_for(&prior_proposal, 7));

        assert!(
            resolve_deferred_chunk_rules(&mut state, &prior_proposal)
                .expect("release prior-round provisional hold")
        );
        assert_eq!(state.rules[0].manifest_hash, None);
        assert_eq!(state.release_pending, VecDeque::from([1]));
    }
    #[test]
    fn target_round_proposal_resolves_rule_and_keeps_matching_chunk_held() {
        let target_proposal = proposal_meta_at(10, 0, 0x33);
        let mut state = State::<NetworkReplyRoute>::default();
        state
            .rules
            .push(deferred_chunk_rule_for(&target_proposal, 10, 0, 7));
        let target_chunk = chunk_meta_for(&target_proposal, 7);
        retain_chunk(&mut state, 1, target_chunk.clone());

        assert!(
            resolve_deferred_chunk_rules(&mut state, &target_proposal)
                .expect("resolve target-round rule")
        );
        assert_eq!(state.rules[0].manifest_hash, target_proposal.manifest_hash);
        assert!(rule_matches_with_proposal_evidence(
            &state.rules[0],
            &target_chunk,
            &state.proposal_round_evidence,
        ));
        assert!(state.release_pending.is_empty());
    }
    #[test]
    fn distinct_proposal_bound_chunk_rounds_resolve_independently() {
        let target_proposal = proposal_meta_at(10, 1, 0x3A);
        let view_zero = deferred_chunk_rule_for(&target_proposal, 10, 0, 7);
        let view_one = deferred_chunk_rule_for(&target_proposal, 10, 1, 7);
        assert!(!view_zero.overlaps(&view_one));

        let resolved_view_zero = Rule {
            manifest_hash: target_proposal.manifest_hash,
            ..view_zero.clone()
        };
        let resolved_view_one = Rule {
            manifest_hash: target_proposal.manifest_hash,
            ..view_one.clone()
        };
        assert!(resolved_view_zero.overlaps(&resolved_view_one));
        let different_resolved_view_one = Rule {
            manifest_hash: Some(manifest_hash(0x3B)),
            ..view_one.clone()
        };
        assert!(!resolved_view_zero.overlaps(&different_resolved_view_one));
        assert!(!view_zero.overlaps(&resolved_view_one));
        let resolved_drop_view_one = Rule {
            action: Action::Drop,
            ..resolved_view_one
        };
        assert!(view_zero.overlaps(&resolved_drop_view_one));

        let mut state = State::<NetworkReplyRoute> {
            rules: vec![view_zero, view_one],
            ..State::default()
        };
        assert!(
            resolve_deferred_chunk_rules(&mut state, &target_proposal)
                .expect("resolve the exact bounded Proposal round")
        );
        assert_eq!(state.rules[0].manifest_hash, None);
        assert_eq!(state.rules[1].manifest_hash, target_proposal.manifest_hash);
        assert!(!state.rules[0].overlaps(&state.rules[1]));
        let target_chunk = chunk_meta_for(&target_proposal, 7);
        assert!(!rule_matches_with_proposal_evidence(
            &state.rules[0],
            &target_chunk,
            &state.proposal_round_evidence,
        ));
        assert!(rule_matches_with_proposal_evidence(
            &state.rules[1],
            &target_chunk,
            &state.proposal_round_evidence,
        ));
    }
    #[test]
    fn command_installation_resolves_deferred_rule_from_prior_proposal_evidence() {
        let target_proposal = proposal_meta_at(10, 0, 0x35);
        let mut state = State::<NetworkReplyRoute>::default();
        assert!(
            !resolve_deferred_chunk_rules(&mut state, &target_proposal)
                .expect("record target Proposal before command")
        );
        let deferred = deferred_chunk_rule_for(&target_proposal, 10, 0, 7);

        apply_command(
            &mut state,
            Command {
                revision: 1,
                queue_capacity: 4,
                rules: vec![deferred],
                release: Vec::new(),
                drain: false,
            },
            Hash::new(b"prior-proposal-command"),
        )
        .expect("install command from exact prior Proposal evidence");

        assert_eq!(state.rules[0].manifest_hash, target_proposal.manifest_hash);
        assert!(rule_matches_with_proposal_evidence(
            &state.rules[0],
            &chunk_meta_for(&target_proposal, 7),
            &state.proposal_round_evidence,
        ));
    }
    #[test]
    fn command_installation_rejects_ambiguous_prior_proposal_evidence() {
        let first = proposal_meta_at(10, 0, 0x36);
        let second = proposal_meta_at(10, 0, 0x37);
        let mut state = State::<NetworkReplyRoute>::default();
        assert!(!resolve_deferred_chunk_rules(&mut state, &first).expect("record first Proposal"));
        assert!(
            !resolve_deferred_chunk_rules(&mut state, &second)
                .expect("record equivocating Proposal")
        );

        let result = apply_command(
            &mut state,
            Command {
                revision: 1,
                queue_capacity: 4,
                rules: vec![deferred_chunk_rule_for(&first, 10, 0, 7)],
                release: Vec::new(),
                drain: false,
            },
            Hash::new(b"ambiguous-prior-proposal-command"),
        );

        assert!(matches!(
            result,
            Err(ControlError::InvalidMessageDescriptor)
        ));
        assert_eq!(state.revision, 0);
        assert!(state.rules.is_empty());
    }
    #[test]
    fn proposal_during_drain_resolves_post_drain_deferred_rule() {
        let target_proposal = proposal_meta_at(10, 0, 0x38);
        let mut state = State::<NetworkReplyRoute>::default();
        retain_chunk(
            &mut state,
            1,
            MessageMeta {
                manifest_hash: Some(manifest_hash(0x39)),
                ..chunk_meta_for(&target_proposal, 7)
            },
        );
        apply_command(
            &mut state,
            Command {
                revision: 1,
                queue_capacity: 4,
                rules: vec![deferred_chunk_rule_for(&target_proposal, 10, 0, 7)],
                release: Vec::new(),
                drain: true,
            },
            Hash::new(b"post-drain-deferred-command"),
        )
        .expect("begin drain with deferred post-fence rule");
        assert_eq!(state.release_pending, VecDeque::from([1]));
        assert_eq!(
            state.drain_next_rules.as_ref().expect("active drain")[0].manifest_hash,
            None
        );

        assert!(
            resolve_deferred_chunk_rules(&mut state, &target_proposal)
                .expect("resolve post-drain rule while drain is active")
        );
        assert_eq!(
            state.drain_next_rules.as_ref().expect("active drain")[0].manifest_hash,
            target_proposal.manifest_hash
        );

        state.held.clear();
        state.held_bytes = 0;
        state.release_pending.clear();
        finish_drain_if_empty(&mut state);
        assert!(state.drain_next_rules.is_none());
        assert_eq!(state.rules[0].manifest_hash, target_proposal.manifest_hash);
    }
    #[test]
    fn proposal_round_evidence_is_fifo_bounded() {
        let mut state = State::<NetworkReplyRoute>::default();
        let base = proposal_meta_at(1, 0, 0x34);
        let manifest_for = |sequence: u64| {
            HashOf::<PayloadManifest>::from_untyped_unchecked(Hash::new(sequence.to_le_bytes()))
        };
        for sequence in
            0..=u64::try_from(MAX_PROPOSAL_ROUND_EVIDENCE).expect("evidence bound fits u64")
        {
            let proposal = MessageMeta {
                height: Some(sequence + 1),
                manifest_hash: Some(manifest_for(sequence)),
                ..base.clone()
            };
            record_proposal_round_evidence(
                &mut state,
                &proposal,
                sequence + 1,
                0,
                manifest_for(sequence),
            )
            .expect("record bounded Proposal evidence");
        }
        assert_eq!(
            state.proposal_round_evidence.len(),
            MAX_PROPOSAL_ROUND_EVIDENCE
        );
        assert_eq!(
            state.proposal_round_evidence_order.len(),
            MAX_PROPOSAL_ROUND_EVIDENCE
        );
        assert!(
            !state
                .proposal_round_evidence
                .contains_key(&ProposalManifestRoute {
                    sender: base.sender.clone(),
                    authenticated_via: base.authenticated_via.clone(),
                    manifest_hash: manifest_for(0),
                })
        );
    }
    #[test]
    fn descriptor_contract_accepts_every_v2_payload_shape() {
        for kind in [
            MessageKind::Proposal,
            MessageKind::PrepareVote,
            MessageKind::CommitVote,
            MessageKind::PrepareCertificate,
            MessageKind::CommitCertificate,
            MessageKind::TimeoutVote,
            MessageKind::TimeoutCertificate,
            MessageKind::PayloadChunk,
            MessageKind::CertifiedBodyRequest,
            MessageKind::CertifiedBodyResponse,
            MessageKind::CommitCertificateRequest,
            MessageKind::CommitCertificateResponse,
            MessageKind::GlobalBeaconPartialSignature,
        ] {
            let meta = valid_meta(kind);
            validate_message_meta(&meta)
                .unwrap_or_else(|error| panic!("valid {kind:?} descriptor failed: {error}"));
            let descriptor = descriptor_value(&HeldDescriptor {
                sequence: 1,
                meta,
                size_bytes: 1,
            })
            .unwrap_or_else(|error| panic!("valid {kind:?} descriptor did not encode: {error}"));
            if kind == MessageKind::CertifiedBodyResponse {
                let descriptor = descriptor.as_object().expect("descriptor object");
                assert_eq!(descriptor.get("signer"), Some(&Value::Null));
                assert_eq!(
                    descriptor.get("cited_responder"),
                    Some(&Value::from(peer(42).to_string()))
                );
            }
        }
    }
    #[test]
    fn descriptor_contract_rejects_unparseable_adversarial_shapes() {
        let mut cases = Vec::new();
        let mut zero_height = valid_meta(MessageKind::PrepareVote);
        zero_height.height = Some(0);
        cases.push(zero_height);
        let mut invalid_commitment = valid_meta(MessageKind::PrepareVote);
        invalid_commitment
            .execution_commitment
            .as_mut()
            .expect("vote commitment")
            .kagemusha_top_up_count = 1;
        cases.push(invalid_commitment);
        let mut mismatched_hash = valid_meta(MessageKind::PrepareVote);
        mismatched_hash.block_hash = Some(hash(99));
        cases.push(mismatched_hash);
        let mut duplicate_signers = valid_meta(MessageKind::PrepareCertificate);
        duplicate_signers.certificate_signers = vec![0, 1, 1];
        cases.push(duplicate_signers);
        let mut empty_certificate = valid_meta(MessageKind::PrepareCertificate);
        empty_certificate.certificate_signers.clear();
        cases.push(empty_certificate);
        let mut invented_chunk_round = valid_meta(MessageKind::PayloadChunk);
        invented_chunk_round.height = Some(9);
        invented_chunk_round.view = Some(2);
        cases.push(invented_chunk_round);
        let mut missing_chunk_manifest = valid_meta(MessageKind::PayloadChunk);
        missing_chunk_manifest.manifest_hash = None;
        cases.push(missing_chunk_manifest);
        let mut missing_chunk_index = valid_meta(MessageKind::PayloadChunk);
        missing_chunk_index.chunk_index = None;
        cases.push(missing_chunk_index);
        let mut missing_vote_signer = valid_meta(MessageKind::PrepareVote);
        missing_vote_signer.signer = None;
        cases.push(missing_vote_signer);
        let mut missing_cited_responder = valid_meta(MessageKind::CertifiedBodyResponse);
        missing_cited_responder.cited_responder = None;
        cases.push(missing_cited_responder);
        let mut false_response_signer = valid_meta(MessageKind::CertifiedBodyResponse);
        false_response_signer.signer = Some(0);
        cases.push(false_response_signer);
        let mut spurious_cited_responder = valid_meta(MessageKind::PrepareVote);
        spurious_cited_responder.cited_responder = Some(peer(42));
        cases.push(spurious_cited_responder);
        for meta in cases {
            assert!(matches!(
                validate_message_meta(&meta),
                Err(ControlError::InvalidMessageDescriptor)
            ));
        }
    }
    #[test]
    fn parser_rejects_malformed_oversized_and_noncanonical_commands() {
        assert!(matches!(
            parse_command(b"{"),
            Err(ControlError::MalformedJson)
        ));
        assert!(matches!(
            parse_command(&vec![b' '; MAX_COMMAND_BYTES + 1]),
            Err(ControlError::CommandTooLarge)
        ));
        let canonical = canonical_json(&object_value([
            ("drain", Value::from(false)),
            ("queue_capacity", Value::from(1_u64)),
            ("release", Value::Array(Vec::new())),
            ("revision", Value::from(1_u64)),
            ("rules", Value::Array(Vec::new())),
            ("version", Value::from(FORMAT_VERSION)),
        ]))
        .expect("canonical JSON");
        let mut noncanonical = canonical;
        noncanonical.push(b'\n');
        assert!(matches!(
            parse_command(&noncanonical),
            Err(ControlError::NonCanonicalJson)
        ));
    }
    #[test]
    fn parser_rejects_zero_height_and_order_dependent_rule_overlap() {
        let mut wildcard = rule(peer(3), MessageKind::PrepareVote, 9, 1);
        wildcard.block_hash = None;
        let specific = rule(peer(3), MessageKind::PrepareVote, 9, 1);
        let command = |rules: Vec<Value>| {
            canonical_json(&object_value([
                ("drain", Value::from(false)),
                ("queue_capacity", Value::from(4_u64)),
                ("release", Value::Array(Vec::new())),
                ("revision", Value::from(1_u64)),
                ("rules", Value::Array(rules)),
                ("version", Value::from(FORMAT_VERSION)),
            ]))
            .expect("canonical command")
        };
        assert!(matches!(
            parse_command(&command(vec![rule_value(&wildcard), rule_value(&specific)])),
            Err(ControlError::AmbiguousRule)
        ));
        let mut relayed = specific.clone();
        relayed.authenticated_via = peer(4);
        assert!(
            parse_command(&command(vec![rule_value(&specific), rule_value(&relayed)])).is_ok(),
            "independent authenticated relays are distinct rule dimensions"
        );
        let mut zero_height = specific.clone();
        zero_height.height = Some(0);
        assert!(matches!(
            parse_command(&command(vec![rule_value(&zero_height)])),
            Err(ControlError::InvalidField("coordinates"))
        ));
        let height_only = Rule {
            sender: peer(5),
            authenticated_via: peer(5),
            kind: MessageKind::CommitCertificateRequest,
            height: Some(9),
            view: None,
            block_hash: None,
            manifest_hash: None,
            chunk_index: None,
            proposal_height: None,
            proposal_view: None,
            action: Action::Hold,
        };
        let parsed = parse_command(&command(vec![rule_value(&height_only)]))
            .expect("parse height-only commit-certificate request rule");
        assert_eq!(parsed.rules, vec![height_only.clone()]);
        let invalid_request_view = Rule {
            view: Some(0),
            ..height_only
        };
        assert!(matches!(
            parse_command(&command(vec![rule_value(&invalid_request_view)])),
            Err(ControlError::InvalidField("coordinates"))
        ));
        let mut invalid_sender = rule_value(&wildcard);
        invalid_sender
            .as_object_mut()
            .expect("rule object")
            .insert("sender".to_owned(), Value::from("not-a-peer"));
        assert!(matches!(
            parse_command(&command(vec![invalid_sender])),
            Err(ControlError::InvalidField("sender"))
        ));
        let mut invalid_via = rule_value(&wildcard);
        invalid_via
            .as_object_mut()
            .expect("rule object")
            .insert("authenticated_via".to_owned(), Value::from("not-a-peer"));
        assert!(matches!(
            parse_command(&command(vec![invalid_via])),
            Err(ControlError::InvalidField("authenticated_via"))
        ));
        let mut uppercase_hash = rule_value(&specific);
        uppercase_hash.as_object_mut().expect("rule object").insert(
            "block_hash".to_owned(),
            Value::from(hash(0xAB).to_string().to_ascii_uppercase()),
        );
        assert!(matches!(
            parse_command(&command(vec![uppercase_hash])),
            Err(ControlError::NonCanonicalField("block_hash"))
        ));
    }
    #[test]
    fn payload_chunk_rule_roundtrips_and_rejects_incompatible_coordinates() {
        let sender = peer(7);
        let chunk_rule = Rule {
            sender: sender.clone(),
            authenticated_via: sender,
            kind: MessageKind::PayloadChunk,
            height: None,
            view: None,
            block_hash: None,
            manifest_hash: Some(manifest_hash(0x55)),
            chunk_index: Some(11),
            proposal_height: None,
            proposal_view: None,
            action: Action::Hold,
        };
        let command_rules = |rules: Vec<Value>| {
            canonical_json(&object_value([
                ("drain", Value::from(false)),
                ("queue_capacity", Value::from(4_u64)),
                ("release", Value::Array(Vec::new())),
                ("revision", Value::from(1_u64)),
                ("rules", Value::Array(rules)),
                ("version", Value::from(FORMAT_VERSION)),
            ]))
            .expect("canonical chunk command")
        };
        let command = |rule: Value| command_rules(vec![rule]);
        let parsed =
            parse_command(&command(rule_value(&chunk_rule))).expect("parse exact chunk rule");
        assert_eq!(parsed.rules, vec![chunk_rule.clone()]);
        assert_eq!(rule_value(&parsed.rules[0]), rule_value(&chunk_rule));

        let mutate = |field: &str, value: Value| {
            let mut encoded = rule_value(&chunk_rule);
            encoded
                .as_object_mut()
                .expect("chunk rule object")
                .insert(field.to_owned(), value);
            command(encoded)
        };
        for invalid in [
            mutate("height", Value::from(9_u64)),
            mutate("view", Value::from(0_u64)),
            mutate("block_hash", Value::from(hash(1).to_string())),
            mutate("manifest_hash", Value::Null),
            mutate("chunk_index", Value::Null),
            mutate("proposal_height", Value::from(9_u64)),
        ] {
            assert!(matches!(
                parse_command(&invalid),
                Err(ControlError::InvalidField("coordinates"))
            ));
        }

        let deferred = Rule {
            manifest_hash: None,
            proposal_height: Some(9),
            proposal_view: Some(2),
            ..chunk_rule.clone()
        };
        let parsed = parse_command(&command(rule_value(&deferred)))
            .expect("parse Proposal-bound deferred chunk rule");
        assert_eq!(parsed.rules, vec![deferred.clone()]);
        let next_view = Rule {
            proposal_view: Some(3),
            ..deferred.clone()
        };
        assert_eq!(
            parse_command(&command_rules(vec![
                rule_value(&deferred),
                rule_value(&next_view),
            ]))
            .expect("parse distinct bounded Proposal views")
            .rules,
            vec![deferred.clone(), next_view]
        );
        assert!(matches!(
            parse_command(&command_rules(vec![
                rule_value(&deferred),
                rule_value(&deferred),
            ])),
            Err(ControlError::AmbiguousRule)
        ));
        let resolved = Rule {
            manifest_hash: Some(manifest_hash(0x55)),
            ..deferred.clone()
        };
        assert!(matches!(
            parse_command(&command(rule_value(&resolved))),
            Err(ControlError::InvalidField("manifest_hash"))
        ));
        let invalid_deferred_drop = Rule {
            action: Action::Drop,
            ..deferred
        };
        assert!(matches!(
            parse_command(&command(rule_value(&invalid_deferred_drop))),
            Err(ControlError::InvalidField("coordinates"))
        ));

        let mut round_with_chunk_coordinates = rule(peer(7), MessageKind::PrepareVote, 9, 0);
        round_with_chunk_coordinates.proposal_height = Some(9);
        round_with_chunk_coordinates.proposal_view = Some(2);
        assert!(matches!(
            parse_command(&command(rule_value(&round_with_chunk_coordinates))),
            Err(ControlError::InvalidField("coordinates"))
        ));
    }
    #[test]
    fn stale_duplicate_reordered_and_unknown_releases_are_atomic() {
        let mut state = State::<NetworkReplyRoute>::default();
        state.revision = 4;
        let original_rules = state.rules.clone();
        assert!(matches!(
            apply_command(
                &mut state,
                Command {
                    revision: 4,
                    queue_capacity: 2,
                    rules: Vec::new(),
                    release: Vec::new(),
                    drain: false,
                },
                Hash::new(b"stale"),
            ),
            Err(ControlError::StaleRevision)
        ));
        for release in [vec![1, 1], vec![2, 1], vec![3]] {
            assert!(validate_release_sequences(&release, [1, 2]).is_err());
        }
        assert_eq!(state.revision, 4);
        assert_eq!(state.rules, original_rules);
        assert!(state.release_pending.is_empty());
    }
    #[test]
    fn hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic() {
        let mut state = State::<NetworkReplyRoute>::default();
        state.queue_capacity = 2;
        state.held_bytes = MAX_HELD_BYTES - 4;
        assert!(hold_capacity_available(&state, 4));
        assert!(!hold_capacity_available(&state, 5));
        state.held_bytes = usize::MAX;
        assert!(!hold_capacity_available(&state, 1));
        state.queue_capacity = 0;
        state.held_bytes = 0;
        assert!(!hold_capacity_available(&state, 0));
        state.queue_capacity = 1;
        state.in_flight = Some(1);
        state.in_flight_bytes = 4;
        assert!(
            !hold_capacity_available(&state, 0),
            "an in-flight release retains its count slot"
        );
        state.queue_capacity = 2;
        state.in_flight_bytes = MAX_HELD_BYTES;
        assert!(
            !hold_capacity_available(&state, 1),
            "an in-flight release retains its exact byte charge"
        );
        state.in_flight = None;
        state.in_flight_bytes = 0;
        fail_hold_overflow(&mut state);
        assert!(state.fatal);
        assert_eq!(state.overflowed, 1);
        assert_eq!(state.last_error.as_deref(), Some("hold_queue_overflow"));
    }
    #[test]
    fn retired_release_finishes_drain_without_claiming_delivery() {
        let (_parent, controller) = test_controller();
        let sender = transport_peer(31);
        let authenticated_via = sender.id().clone();
        controller.drain_subsequent_messages_for_tests();
        controller
            .admit(sender, &authenticated_via, chunk_message(1), 101)
            .expect("hold one drain occurrence");
        let released = controller
            .next_release()
            .expect("take release")
            .expect("held occurrence is releasable");
        {
            let state = controller.state.lock().expect("control state");
            assert_eq!(state.held_bytes, 0);
            assert_eq!(state.in_flight, Some(released.sequence));
            assert_eq!(state.in_flight_bytes, 101);
        }
        controller
            .complete_release(released.sequence, ReleaseOutcome::Retired)
            .expect("retirement is a successful terminal release");
        let state = controller.state.lock().expect("control state");
        assert!(!state.fatal);
        assert!(state.delivered.is_empty());
        assert_eq!(state.retired, vec![released.sequence]);
        assert_eq!(state.in_flight, None);
        assert_eq!(state.in_flight_bytes, 0);
        assert!(state.drain_next_rules.is_none());
    }
    #[test]
    fn failed_release_clears_in_flight_ownership_and_latches_fatal() {
        let (_parent, controller) = test_controller();
        let sender = transport_peer(32);
        let authenticated_via = sender.id().clone();
        controller.drain_subsequent_messages_for_tests();
        controller
            .admit(sender, &authenticated_via, chunk_message(1), 202)
            .expect("hold one failing occurrence");
        let released = controller
            .next_release()
            .expect("take release")
            .expect("held occurrence is releasable");
        assert!(matches!(
            controller.complete_release(released.sequence, ReleaseOutcome::Failed),
            Err(ControlError::DownstreamDeliveryFailed)
        ));
        let state = controller.state.lock().expect("control state");
        assert!(state.fatal);
        assert!(state.delivered.is_empty());
        assert!(state.retired.is_empty());
        assert_eq!(state.in_flight, None);
        assert_eq!(state.in_flight_bytes, 0);
        assert!(state.drain_next_rules.is_some());
    }
    #[test]
    fn fatal_controller_rejects_an_unchanged_command_poll() {
        let (_control_dir, controller) = test_controller();
        let bytes = canonical_json(&object_value([
            ("drain", Value::from(false)),
            ("queue_capacity", Value::from(1_u64)),
            ("release", Value::Array(Vec::new())),
            ("revision", Value::from(1_u64)),
            ("rules", Value::Array(Vec::new())),
            ("version", Value::from(FORMAT_VERSION)),
        ]))
        .expect("canonical command");
        write_atomic_private_file(&controller.root, CONTROL_FILE, &bytes)
            .expect("install private command");
        controller.poll_command().expect("initial command poll");
        {
            let mut state = controller
                .state
                .lock()
                .expect("message control state poisoned");
            state.fatal = true;
            state.last_error = Some("test_fatal".to_owned());
        }
        assert!(matches!(
            controller.poll_command(),
            Err(ControlError::ControllerFatal)
        ));
    }
    #[test]
    fn drain_fence_holds_racing_chunks_fifo_until_atomic_cutover() {
        let (_parent, controller) = test_controller();
        let sender = transport_peer(11);
        let authenticated_via = sender.id().clone();
        // Seed one retained pre-fence chunk. Compact chunks deliberately have
        // no fabricated height/view metadata.
        {
            let mut state = controller.state.lock().expect("control state");
            state.drain_next_rules = Some(Vec::new());
        }
        assert_eq!(
            controller
                .admit(sender.clone(), &authenticated_via, chunk_message(1), 101,)
                .expect("seed retained chunk")
                .0,
            Admission::Held
        );
        {
            let mut state = controller.state.lock().expect("control state");
            let descriptor = &state.held.get(&1).expect("seed descriptor").descriptor;
            assert_eq!(descriptor.meta.sender, authenticated_via);
            assert_eq!(descriptor.meta.authenticated_via, authenticated_via);
            assert_eq!(descriptor.meta.signer, Some(0));
            assert!(descriptor.meta.subject.is_none());
            assert!(descriptor.meta.execution_commitment.is_none());
            assert_ne!(descriptor.meta.envelope_digest, Hash::new(b""));
            state.drain_next_rules = None;
            state.release_pending.clear();
        }
        let next_rules = vec![Rule {
            sender: sender.id().clone(),
            authenticated_via: sender.id().clone(),
            kind: MessageKind::PrepareVote,
            height: Some(9),
            view: Some(2),
            block_hash: None,
            manifest_hash: None,
            chunk_index: None,
            proposal_height: None,
            proposal_view: None,
            action: Action::Drop,
        }];
        {
            let mut state = controller.state.lock().expect("control state");
            apply_command(
                &mut state,
                Command {
                    revision: 1,
                    queue_capacity: 8,
                    rules: next_rules.clone(),
                    release: Vec::new(),
                    drain: true,
                },
                Hash::new(b"drain-fence"),
            )
            .expect("begin drain");
            assert_eq!(state.drain_fence, Some(1));
            assert!(state.drain_next_rules.is_some());
            assert!(state.rules.is_empty());
        }
        let first = controller
            .next_release()
            .expect("take first release")
            .expect("first release");
        assert_eq!(first.sequence, 1);
        std::thread::scope(|scope| {
            for marker in [2_u8, 3] {
                let controller = &controller;
                let sender = sender.clone();
                let authenticated_via = authenticated_via.clone();
                scope.spawn(move || {
                    assert_eq!(
                        controller
                            .admit(
                                sender,
                                &authenticated_via,
                                chunk_message(marker),
                                100 + usize::from(marker),
                            )
                            .expect("admit racing chunk")
                            .0,
                        Admission::Held
                    );
                });
            }
        });
        controller
            .complete_release(first.sequence, ReleaseOutcome::Delivered)
            .expect("complete first release");
        let mut released = vec![first.sequence];
        while let Some(message) = controller.next_release().expect("take drain release") {
            released.push(message.sequence);
            controller
                .complete_release(message.sequence, ReleaseOutcome::Delivered)
                .expect("complete drain release");
        }
        assert_eq!(released, vec![1, 2, 3]);
        {
            let state = controller.state.lock().expect("control state");
            assert!(state.drain_next_rules.is_none());
            assert_eq!(state.drain_fence, Some(1));
            assert_eq!(state.rules, next_rules);
            assert!(state.held.is_empty());
            assert!(state.release_pending.is_empty());
            assert!(state.in_flight.is_none());
        }
        assert_eq!(
            controller
                .admit(sender, &authenticated_via, chunk_message(4), 104)
                .expect("post-fence admission")
                .0,
            Admission::Pass
        );
    }
    #[test]
    fn controlled_v2_admission_preserves_distinct_relay_identity() {
        let (_parent, controller) = test_controller();
        let semantic_sender = transport_peer(21);
        let authenticated_via = peer(22);
        {
            let mut state = controller.state.lock().expect("control state");
            state.drain_next_rules = Some(Vec::new());
        }
        let result = controller
            .admit(
                semantic_sender.clone(),
                &authenticated_via,
                chunk_message(1),
                101,
            )
            .expect("relay-authenticated message must remain controllable");
        assert_eq!(result.0, Admission::Held);
        let state = controller.state.lock().expect("control state");
        assert!(!state.fatal);
        let held = state.held.get(&1).expect("relayed message is retained");
        assert_eq!(held.descriptor.meta.sender, semantic_sender.id().clone());
        assert_eq!(held.descriptor.meta.authenticated_via, authenticated_via);
        assert_eq!(held.authenticated_via, authenticated_via);
        drop(state);
        let released = controller
            .next_release()
            .expect("take relayed message release")
            .expect("relayed message remains releasable");
        assert_eq!(released.peer, semantic_sender);
        assert_eq!(released.authenticated_via, authenticated_via);
        assert!(released.reply_route.is_none());
        controller
            .complete_release(released.sequence, ReleaseOutcome::Delivered)
            .expect("complete relayed message release");
    }
    #[test]
    fn direct_private_reader_rejects_symlink_and_hardlink_commands() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source");
        fs::write(&source, b"{}").expect("write source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).expect("chmod source");
        let hardlink = directory.path().join("hardlink");
        fs::hard_link(&source, &hardlink).expect("create hardlink");
        assert!(matches!(
            read_stable_private_file(&hardlink, MAX_COMMAND_BYTES),
            Err(ControlError::UnsafePermissions)
        ));
        let symlink_path = directory.path().join("symlink");
        symlink(&source, &symlink_path).expect("create symlink");
        assert!(matches!(
            read_stable_private_file(&symlink_path, MAX_COMMAND_BYTES),
            Err(ControlError::UnsafeFile)
        ));
    }
    #[test]
    fn private_reader_treats_safe_atomic_replacement_as_retryable_identity_churn() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempdir().expect("temporary directory");
        let command = directory.path().join("command.json");
        let replacement = directory.path().join("replacement.json");
        fs::write(&command, b"{}").expect("write original command");
        fs::write(&replacement, b"{}").expect("write replacement command");
        fs::set_permissions(&command, fs::Permissions::from_mode(0o600))
            .expect("chmod original command");
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600))
            .expect("chmod replacement command");
        let error = read_stable_private_file_after_open(&command, MAX_COMMAND_BYTES, || {
            fs::rename(&replacement, &command).expect("atomically replace command");
        })
        .expect_err("an atomic pathname replacement must be retried");
        assert!(
            matches!(error, ControlError::FileIdentityChanged),
            "safe replacement must be retryable identity churn, got {error:?}"
        );
    }
    #[test]
    fn stable_private_reader_retries_identity_churn_once_but_not_unsafe_permissions() {
        use std::cell::Cell;

        let attempts = Cell::new(0_u8);
        let bytes = retry_file_identity_change_once(|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Err(ControlError::FileIdentityChanged)
            } else {
                Ok(b"stable".to_vec())
            }
        })
        .expect("one identity replacement is retried");
        assert_eq!(bytes.as_slice(), b"stable");
        assert_eq!(attempts.get(), 2, "exactly one retry is permitted");

        let repeated_attempts = Cell::new(0_u8);
        assert!(matches!(
            retry_file_identity_change_once(|| {
                repeated_attempts.set(repeated_attempts.get() + 1);
                Err(ControlError::FileIdentityChanged)
            }),
            Err(ControlError::FileIdentityChanged)
        ));
        assert_eq!(
            repeated_attempts.get(),
            2,
            "a second identity change propagates instead of retrying again"
        );

        let unsafe_attempts = Cell::new(0_u8);
        assert!(matches!(
            retry_file_identity_change_once(|| {
                unsafe_attempts.set(unsafe_attempts.get() + 1);
                Err(ControlError::UnsafePermissions)
            }),
            Err(ControlError::UnsafePermissions)
        ));
        assert_eq!(
            unsafe_attempts.get(),
            1,
            "unsafe metadata must fail closed without retry"
        );
    }
}
