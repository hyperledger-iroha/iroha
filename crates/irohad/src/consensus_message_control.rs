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

use iroha_core::NetworkMessage;
use iroha_crypto::Hash;
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus_v2::{ConsensusMessageV2Payload, GlobalPhase},
    },
    peer::{Peer, PeerId},
};
use norito::json::{Map, Value};

/// Environment variable consumed only by the feature-isolated test daemon.
pub(crate) const CONTROL_DIR_ENV: &str = "IROHA_TEST_CONSENSUS_MESSAGE_CONTROL_DIR";

const CONTROL_FILE: &str = "command.norito.json";
const ACK_FILE: &str = "ack.norito.json";
const FORMAT_VERSION: u64 = 1;
const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_ACK_BYTES: usize = 1024 * 1024;
const MAX_RULES: usize = 256;
const MAX_HOLDS: usize = 1_024;
const MAX_HELD_BYTES: usize = 64 * 1024 * 1024;
const MAX_RELEASES: usize = MAX_HOLDS;
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
    PayloadManifest,
    PayloadChunk,
    CertifiedBodyRequest,
    CertifiedBodyResponse,
    CommitCertificateRequest,
    CommitCertificateResponse,
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
            "payload_manifest" => Ok(Self::PayloadManifest),
            "payload_chunk" => Ok(Self::PayloadChunk),
            "certified_body_request" => Ok(Self::CertifiedBodyRequest),
            "certified_body_response" => Ok(Self::CertifiedBodyResponse),
            "commit_certificate_request" => Ok(Self::CommitCertificateRequest),
            "commit_certificate_response" => Ok(Self::CommitCertificateResponse),
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
            Self::PayloadManifest => "payload_manifest",
            Self::PayloadChunk => "payload_chunk",
            Self::CertifiedBodyRequest => "certified_body_request",
            Self::CertifiedBodyResponse => "certified_body_response",
            Self::CommitCertificateRequest => "commit_certificate_request",
            Self::CommitCertificateResponse => "commit_certificate_response",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MessageMeta {
    sender: PeerId,
    kind: MessageKind,
    height: Option<u64>,
    view: Option<u64>,
    block_hash: Option<HashOf<BlockHeader>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Rule {
    sender: PeerId,
    kind: MessageKind,
    height: u64,
    view: u64,
    block_hash: Option<HashOf<BlockHeader>>,
    action: Action,
}

impl Rule {
    fn matches(&self, meta: &MessageMeta) -> bool {
        self.sender == meta.sender
            && self.kind == meta.kind
            && Some(self.height) == meta.height
            && Some(self.view) == meta.view
            && self
                .block_hash
                .as_ref()
                .is_none_or(|expected| meta.block_hash.as_ref() == Some(expected))
    }
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

pub(crate) struct HeldMessage {
    pub(crate) sequence: u64,
    pub(crate) peer: Peer,
    pub(crate) message: NetworkMessage,
    pub(crate) size_bytes: usize,
}

struct HeldEntry {
    descriptor: HeldDescriptor,
    peer: Peer,
    message: NetworkMessage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    Pass,
    Consumed,
}

struct State {
    revision: u64,
    command_digest: Option<Hash>,
    last_seen_digest: Option<Hash>,
    rules: Vec<Rule>,
    queue_capacity: usize,
    held: BTreeMap<u64, HeldEntry>,
    held_bytes: usize,
    release_pending: VecDeque<u64>,
    in_flight: Option<u64>,
    delivered: Vec<u64>,
    next_sequence: u64,
    dropped: u64,
    overflowed: u64,
    rejected_commands: u64,
    last_error: Option<String>,
    fatal: bool,
    drain_next_rules: Option<Vec<Rule>>,
    drain_fence: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            revision: 0,
            command_digest: None,
            last_seen_digest: None,
            rules: Vec::new(),
            queue_capacity: MAX_HOLDS,
            held: BTreeMap::new(),
            held_bytes: 0,
            release_pending: VecDeque::new(),
            in_flight: None,
            delivered: Vec::new(),
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
pub(crate) struct Controller {
    root: PathBuf,
    root_identity: RootIdentity,
    state: Mutex<State>,
    ack_publish: Mutex<()>,
}

impl Controller {
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
        message: NetworkMessage,
        size_bytes: usize,
    ) -> Result<(Admission, Option<(Peer, NetworkMessage, usize)>), ControlError> {
        let Some(meta) = message_meta(&peer, &message) else {
            return Ok((Admission::Pass, Some((peer, message, size_bytes))));
        };
        let mut state = self.state.lock().expect("message control state poisoned");
        if state.fatal {
            return Ok((Admission::Consumed, None));
        }
        let draining = state.drain_next_rules.is_some();
        let action = if draining {
            Action::Hold
        } else if let Some(action) = state
            .rules
            .iter()
            .find(|rule| rule.matches(&meta))
            .map(|rule| rule.action)
        {
            action
        } else {
            return Ok((Admission::Pass, Some((peer, message, size_bytes))));
        };
        match action {
            Action::Drop => {
                state.dropped = state.dropped.saturating_add(1);
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
                            message,
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
        Ok((Admission::Consumed, None))
    }

    /// Take the next prevalidated release entry in exact ingress order.
    pub(crate) fn next_release(&self) -> Result<Option<HeldMessage>, ControlError> {
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
        drop(state);
        self.publish_ack()?;
        Ok(Some(HeldMessage {
            sequence,
            peer: entry.peer,
            message: entry.message,
            size_bytes: entry.descriptor.size_bytes,
        }))
    }

    /// Record exact completion of one release. Failure permanently closes the controller.
    pub(crate) fn complete_release(
        &self,
        sequence: u64,
        delivered: bool,
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
        if delivered {
            state.delivered.push(sequence);
            finish_drain_if_empty(&mut state);
        } else {
            state.fatal = true;
            state.last_error = Some("downstream_delivery_failed".to_owned());
        }
        drop(state);
        self.publish_ack()?;
        if delivered {
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
            canonical_json(&ack_value(&state))?
        };
        if bytes.len() > MAX_ACK_BYTES {
            return Err(ControlError::AckTooLarge);
        }
        write_atomic_private_file(&self.root, ACK_FILE, &bytes)?;
        self.validate_root()
    }
}

fn hold_capacity_available(state: &State, incoming_bytes: usize) -> bool {
    state.held.len() < state.queue_capacity
        && state
            .held_bytes
            .checked_add(incoming_bytes)
            .is_some_and(|bytes| bytes <= MAX_HELD_BYTES)
}

fn fail_hold_overflow(state: &mut State) {
    state.overflowed = state.overflowed.saturating_add(1);
    state.fatal = true;
    state.last_error = Some("hold_queue_overflow".to_owned());
}

fn apply_command(state: &mut State, command: Command, digest: Hash) -> Result<(), ControlError> {
    if state.fatal {
        return Err(ControlError::ControllerFatal);
    }
    if command.revision <= state.revision {
        return Err(ControlError::StaleRevision);
    }
    if state.in_flight.is_some() || !state.release_pending.is_empty() {
        return Err(ControlError::ReleaseBusy);
    }
    if command.queue_capacity < state.held.len() {
        return Err(ControlError::QueueCapacityBelowHeld);
    }
    if command.drain && !command.release.is_empty() {
        return Err(ControlError::DrainWithExplicitRelease);
    }
    validate_release_sequences(&command.release, state.held.keys().copied())?;

    state.revision = command.revision;
    state.command_digest = Some(digest);
    state.queue_capacity = command.queue_capacity;
    state.delivered.clear();
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
/// retained and in-flight messages delivered.
fn finish_drain_if_empty(state: &mut State) {
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
        if rules.iter().any(|prior: &Rule| {
            prior.sender == rule.sender
                && prior.kind == rule.kind
                && prior.height == rule.height
                && prior.view == rule.view
                && (prior.block_hash.is_none()
                    || rule.block_hash.is_none()
                    || prior.block_hash == rule.block_hash)
        }) {
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
        &["action", "block_hash", "height", "kind", "sender", "view"],
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
    if matches!(
        kind,
        MessageKind::PayloadChunk | MessageKind::CommitCertificateRequest
    ) {
        return Err(ControlError::KindHasNoExactRound);
    }
    Ok(Rule {
        sender,
        kind,
        height: {
            let height = required_u64(object, "height")?;
            if height == 0 {
                return Err(ControlError::InvalidField("height"));
            }
            height
        },
        view: required_u64(object, "view")?,
        block_hash,
        action: Action::parse(
            object
                .get("action")
                .ok_or(ControlError::InvalidField("action"))?,
        )?,
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

fn canonical_json(value: &Value) -> Result<Vec<u8>, ControlError> {
    norito::json::to_json(value)
        .map(String::into_bytes)
        .map_err(|_| ControlError::JsonEncode)
}

fn ack_value(state: &State) -> Value {
    let held = state
        .held
        .values()
        .map(|entry| descriptor_value(&entry.descriptor))
        .collect::<Vec<_>>();
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
    object_value([
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
        ("revision", Value::from(state.revision)),
        ("rules", Value::Array(rules)),
        ("version", Value::from(FORMAT_VERSION)),
    ])
}

fn descriptor_value(descriptor: &HeldDescriptor) -> Value {
    object_value([
        (
            "block_hash",
            descriptor
                .meta
                .block_hash
                .as_ref()
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        (
            "height",
            descriptor.meta.height.map_or(Value::Null, Value::from),
        ),
        ("kind", Value::from(descriptor.meta.kind.as_str())),
        ("sender", Value::from(descriptor.meta.sender.to_string())),
        ("sequence", Value::from(descriptor.sequence)),
        (
            "size_bytes",
            Value::from(u64::try_from(descriptor.size_bytes).unwrap_or(u64::MAX)),
        ),
        (
            "view",
            descriptor.meta.view.map_or(Value::Null, Value::from),
        ),
    ])
}

fn rule_value(rule: &Rule) -> Value {
    object_value([
        ("action", Value::from(rule.action.as_str())),
        (
            "block_hash",
            rule.block_hash
                .as_ref()
                .map_or(Value::Null, |hash| Value::from(hash.to_string())),
        ),
        ("height", Value::from(rule.height)),
        ("kind", Value::from(rule.kind.as_str())),
        ("sender", Value::from(rule.sender.to_string())),
        ("view", Value::from(rule.view)),
    ])
}

fn object_value<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut object = Map::new();
    for (key, value) in entries {
        object.insert(key.to_owned(), value);
    }
    Value::Object(object)
}

fn message_meta(peer: &Peer, message: &NetworkMessage) -> Option<MessageMeta> {
    let NetworkMessage::SumeragiBlock(block) = message else {
        return None;
    };
    let iroha_core::sumeragi::message::BlockMessage::V2(message) = block.as_ref().as_ref() else {
        return None;
    };
    let sender = peer.id().clone();
    let (kind, round, block_hash) = match &message.payload {
        ConsensusMessageV2Payload::Proposal(value) => (
            MessageKind::Proposal,
            Some(value.round),
            Some(value.subject.block_hash),
        ),
        ConsensusMessageV2Payload::Vote(value) => (
            match value.phase {
                GlobalPhase::Prepare => MessageKind::PrepareVote,
                GlobalPhase::Commit => MessageKind::CommitVote,
            },
            Some(value.round),
            Some(value.subject.block_hash),
        ),
        ConsensusMessageV2Payload::QuorumCertificate(value) => (
            match value.phase {
                GlobalPhase::Prepare => MessageKind::PrepareCertificate,
                GlobalPhase::Commit => MessageKind::CommitCertificate,
            },
            Some(value.round),
            Some(value.subject.block_hash),
        ),
        ConsensusMessageV2Payload::TimeoutVote(value) => (
            MessageKind::TimeoutVote,
            Some(value.round),
            value
                .highest_prepare_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
        ),
        ConsensusMessageV2Payload::TimeoutCertificate(value) => (
            MessageKind::TimeoutCertificate,
            Some(value.round),
            value.highest_prepare_qc().map(|qc| qc.subject.block_hash),
        ),
        ConsensusMessageV2Payload::PayloadManifest(value) => (
            MessageKind::PayloadManifest,
            Some(value.round),
            Some(value.subject.block_hash),
        ),
        ConsensusMessageV2Payload::PayloadChunk(_) => (MessageKind::PayloadChunk, None, None),
        ConsensusMessageV2Payload::CertifiedBodyRequest(value) => (
            MessageKind::CertifiedBodyRequest,
            Some(value.round),
            Some(value.subject.block_hash),
        ),
        ConsensusMessageV2Payload::CertifiedBodyResponse(value) => (
            MessageKind::CertifiedBodyResponse,
            Some(value.manifest.round),
            Some(value.manifest.subject.block_hash),
        ),
        ConsensusMessageV2Payload::CommitCertificateRequest(value) => {
            return Some(MessageMeta {
                sender,
                kind: MessageKind::CommitCertificateRequest,
                height: Some(value.height),
                view: None,
                block_hash: None,
            });
        }
        ConsensusMessageV2Payload::CommitCertificateResponse(value) => (
            MessageKind::CommitCertificateResponse,
            Some(value.certificate.round),
            Some(value.certificate.subject.block_hash),
        ),
    };
    Some(MessageMeta {
        sender,
        kind,
        height: round.map(|round| round.height),
        view: round.map(|round| round.view),
        block_hash,
    })
}

fn read_stable_private_file(path: &Path, max_bytes: usize) -> Result<Vec<u8>, ControlError> {
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
    let opened_before = file.metadata().map_err(ControlError::Io)?;
    validate_private_file(&opened_before)?;
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
    validate_private_file(&opened_after)?;
    validate_private_file(&named_after)?;
    if !same_file(&opened_before, &opened_after)
        || !same_file(&opened_after, &named_after)
        || opened_before.len() != opened_after.len()
        || opened_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || opened_before.modified().ok() != opened_after.modified().ok()
    {
        return Err(ControlError::FileIdentityChanged);
    }
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
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ControlError::UnsafeFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o777 != 0o600
            || metadata.nlink() != 1
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
    KindHasNoExactRound,
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
            Self::KindHasNoExactRound => "kind_has_no_exact_round",
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
    use tempfile::tempdir;

    fn peer(marker: u8) -> PeerId {
        let key = KeyPair::try_from_seed(vec![marker; 32], Algorithm::Ed25519)
            .expect("deterministic peer key");
        PeerId::new(key.public_key().clone())
    }

    fn hash(marker: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([marker; Hash::LENGTH]))
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
        NetworkMessage::SumeragiBlock(Box::new(BlockMessageWire::new(BlockMessage::V2(
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
        use std::os::unix::fs::PermissionsExt;

        let parent = tempdir().expect("temporary parent");
        let root = parent.path().join("control");
        fs::create_dir(&root).expect("create control root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("chmod control root");
        let metadata = fs::symlink_metadata(&root).expect("control metadata");
        let controller = Controller {
            root,
            root_identity: root_identity(&metadata),
            state: Mutex::new(State::default()),
            ack_publish: Mutex::new(()),
        };
        (parent, controller)
    }

    fn rule(sender: PeerId, kind: MessageKind, height: u64, view: u64) -> Rule {
        Rule {
            sender,
            kind,
            height,
            view,
            block_hash: Some(hash(1)),
            action: Action::Hold,
        }
    }

    #[test]
    fn matcher_is_exact_across_every_authenticated_dimension() {
        let peer_a = peer(1);
        let rule = rule(peer_a.clone(), MessageKind::PrepareVote, 9, 3);
        let exact = MessageMeta {
            sender: peer_a,
            kind: MessageKind::PrepareVote,
            height: Some(9),
            view: Some(3),
            block_hash: Some(hash(1)),
        };
        assert!(rule.matches(&exact));
        for changed in [
            MessageMeta {
                sender: peer(2),
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
        let mut zero_height = specific;
        zero_height.height = 0;
        assert!(matches!(
            parse_command(&command(vec![rule_value(&zero_height)])),
            Err(ControlError::InvalidField("height"))
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
        let mut uppercase_hash = rule_value(&specific);
        uppercase_hash.as_object_mut().expect("rule object").insert(
            "block_hash".to_owned(),
            Value::from(hash(1).to_string().to_ascii_uppercase()),
        );
        assert!(matches!(
            parse_command(&command(vec![uppercase_hash])),
            Err(ControlError::NonCanonicalField("block_hash"))
        ));
    }

    #[test]
    fn stale_duplicate_reordered_and_unknown_releases_are_atomic() {
        let mut state = State::default();
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
        let mut state = State::default();
        state.queue_capacity = 2;
        state.held_bytes = MAX_HELD_BYTES - 4;
        assert!(hold_capacity_available(&state, 4));
        assert!(!hold_capacity_available(&state, 5));
        state.held_bytes = usize::MAX;
        assert!(!hold_capacity_available(&state, 1));
        state.queue_capacity = 0;
        state.held_bytes = 0;
        assert!(!hold_capacity_available(&state, 0));
        fail_hold_overflow(&mut state);
        assert!(state.fatal);
        assert_eq!(state.overflowed, 1);
        assert_eq!(state.last_error.as_deref(), Some("hold_queue_overflow"));
    }

    #[test]
    fn drain_fence_holds_racing_chunks_fifo_until_atomic_cutover() {
        let (_parent, controller) = test_controller();
        let sender = transport_peer(11);

        // Seed one retained pre-fence chunk. Compact chunks deliberately have
        // no fabricated height/view metadata.
        {
            let mut state = controller.state.lock().expect("control state");
            state.drain_next_rules = Some(Vec::new());
        }
        assert_eq!(
            controller
                .admit(sender.clone(), chunk_message(1), 101)
                .expect("seed retained chunk")
                .0,
            Admission::Consumed
        );
        {
            let mut state = controller.state.lock().expect("control state");
            state.drain_next_rules = None;
            state.release_pending.clear();
        }

        let next_rules = vec![Rule {
            sender: sender.id().clone(),
            kind: MessageKind::PrepareVote,
            height: 9,
            view: 2,
            block_hash: None,
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
                scope.spawn(move || {
                    assert_eq!(
                        controller
                            .admit(sender, chunk_message(marker), 100 + usize::from(marker))
                            .expect("admit racing chunk")
                            .0,
                        Admission::Consumed
                    );
                });
            }
        });
        controller
            .complete_release(first.sequence, true)
            .expect("complete first release");

        let mut released = vec![first.sequence];
        while let Some(message) = controller.next_release().expect("take drain release") {
            released.push(message.sequence);
            controller
                .complete_release(message.sequence, true)
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
                .admit(sender, chunk_message(4), 104)
                .expect("post-fence admission")
                .0,
            Admission::Pass
        );
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
}
