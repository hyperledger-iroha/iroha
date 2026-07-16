//! Replays TLC-generated and adversarial schedules against the production reducer.
//!
//! TLC's tool-mode trace is normalized by
//! `scripts/normalize_sumeragi_v2_tlc_trace.py`. The checked-in witness
//! preserves every selected Core-enabled model action
//! while this harness maps those actions onto the reducer's serialized
//! event/effect API.
//! The mapping is deliberately strict: malformed traces, invalid leaders,
//! certificates without delivered quorum votes, and out-of-order durability
//! boundaries are rejected before replay.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use iroha_sumeragi_core::{
    BodyState, CertificateRef, ChainId, ConsensusMessageV2, ContextId, Digest, Effect,
    EquivocationKind, Event, EventTag, Generation, HeightContext, IgnoreReason, OpaqueSignature,
    PayloadManifest, Phase, QuorumError, Reducer, ReducerError, Round, SignableMessage,
    SignatureShare, SignedTimeoutVote, SignedVote, StepDisposition, Subject, TimeoutCertificate,
    TimeoutSignatureGroup, TimeoutVote, Validator, ValidatorId, Vote, VotingMode, VotingPower,
    WalEntry, WalRecord,
};

const TRACE: &str = include_str!("fixtures/tlc_replay_witness.tsv");
const HEIGHT: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ModelSubject {
    A,
    B,
}

impl ModelSubject {
    fn parse(value: &str) -> Result<Option<Self>, String> {
        match value {
            "-" => Ok(None),
            "A" => Ok(Some(Self::A)),
            "B" => Ok(Some(Self::B)),
            _ => Err(format!("unknown model subject {value:?}")),
        }
    }

    const fn production(self) -> Subject {
        match self {
            Self::A => Subject::repeat(0xa1),
            Self::B => Subject::repeat(0xb2),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ModelAction {
    SetGst,
    AssembleLocalBody,
    BeginTimeout,
    PersistTimeout,
    CompleteTimeoutSignature,
    DeliverTimeout,
    FormTc,
    PersistInstallTc,
    DeliverTc,
    BeginInstallTc,
    BeginLocalProposal,
    PersistProposal,
    CompleteProposalSignature,
    DeliverProposal,
    FetchBody,
    StoreBody,
    ValidateBody,
    BeginPrepare,
    PersistPrepare,
    CompleteVoteSignature,
    DeliverVote,
    FormPrepareQc,
    DeliverQc,
    BeginObservePrepare,
    PersistObservePrepare,
    BeginLockCommit,
    PersistLockCommit,
    FormCommitQc,
    PersistDecision,
}

impl ModelAction {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "SetGST" => Ok(Self::SetGst),
            "AssembleLocalBody" => Ok(Self::AssembleLocalBody),
            "BeginTimeout" => Ok(Self::BeginTimeout),
            "PersistTimeout" => Ok(Self::PersistTimeout),
            "CompleteTimeoutSignature" => Ok(Self::CompleteTimeoutSignature),
            "DeliverTimeout" => Ok(Self::DeliverTimeout),
            "FormTC" => Ok(Self::FormTc),
            "PersistInstallTC" => Ok(Self::PersistInstallTc),
            "DeliverTC" => Ok(Self::DeliverTc),
            "BeginInstallTC" => Ok(Self::BeginInstallTc),
            "BeginLocalProposal" => Ok(Self::BeginLocalProposal),
            "PersistProposal" => Ok(Self::PersistProposal),
            "CompleteProposalSignature" => Ok(Self::CompleteProposalSignature),
            "DeliverProposal" => Ok(Self::DeliverProposal),
            "FetchBody" => Ok(Self::FetchBody),
            "StoreBody" => Ok(Self::StoreBody),
            "ValidateBody" => Ok(Self::ValidateBody),
            "BeginPrepare" => Ok(Self::BeginPrepare),
            "PersistPrepare" => Ok(Self::PersistPrepare),
            "CompleteVoteSignature" => Ok(Self::CompleteVoteSignature),
            "DeliverVote" => Ok(Self::DeliverVote),
            "FormPrepareQC" => Ok(Self::FormPrepareQc),
            "DeliverQC" => Ok(Self::DeliverQc),
            "BeginObservePrepare" => Ok(Self::BeginObservePrepare),
            "PersistObservePrepare" => Ok(Self::PersistObservePrepare),
            "BeginLockCommit" => Ok(Self::BeginLockCommit),
            "PersistLockCommit" => Ok(Self::PersistLockCommit),
            "FormCommitQC" => Ok(Self::FormCommitQc),
            "PersistDecision" => Ok(Self::PersistDecision),
            _ => Err(format!("unknown model action {value:?}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ModelStep {
    number: usize,
    action: ModelAction,
    node: Option<usize>,
    peer: Option<usize>,
    view: Option<u64>,
    phase: Option<Phase>,
    subject: Option<ModelSubject>,
}

fn parse_optional_usize(value: &str, field: &str) -> Result<Option<usize>, String> {
    if value == "-" {
        return Ok(None);
    }
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid {field} {value:?}"))?;
    if parsed >= 4 {
        return Err(format!("{field} {parsed} is outside the four-node model"));
    }
    Ok(Some(parsed))
}

fn parse_optional_view(value: &str) -> Result<Option<u64>, String> {
    if value == "-" {
        Ok(None)
    } else {
        value
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("invalid view {value:?}"))
    }
}

fn parse_optional_phase(value: &str) -> Result<Option<Phase>, String> {
    match value {
        "-" => Ok(None),
        "Prepare" => Ok(Some(Phase::Prepare)),
        "Commit" => Ok(Some(Phase::Commit)),
        _ => Err(format!("unknown phase {value:?}")),
    }
}

fn parse_trace(input: &str) -> Result<Vec<ModelStep>, String> {
    let mut steps = Vec::new();
    for (line_index, line) in input.lines().enumerate() {
        let line_number = line_index + 1;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 7 {
            return Err(format!(
                "trace line {line_number} has {} columns, expected 7",
                columns.len()
            ));
        }
        let number = columns[0]
            .parse::<usize>()
            .map_err(|_| format!("trace line {line_number} has an invalid step"))?;
        if number != steps.len() + 1 {
            return Err(format!(
                "trace step {number} is non-contiguous; expected {}",
                steps.len() + 1
            ));
        }
        steps.push(ModelStep {
            number,
            action: ModelAction::parse(columns[1])?,
            node: parse_optional_usize(columns[2], "node")?,
            peer: parse_optional_usize(columns[3], "peer")?,
            view: parse_optional_view(columns[4])?,
            phase: parse_optional_phase(columns[5])?,
            subject: ModelSubject::parse(columns[6])?,
        });
    }
    if steps.is_empty() {
        return Err("trace contains no actions".to_owned());
    }
    validate_model_trace(&steps)?;
    Ok(steps)
}

fn required<T: Copy>(value: Option<T>, step: ModelStep, field: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("step {} {:?} is missing {field}", step.number, step.action))
}

#[allow(clippy::too_many_lines)]
fn validate_model_trace(steps: &[ModelStep]) -> Result<(), String> {
    if steps.first().map(|step| step.action) != Some(ModelAction::SetGst) {
        return Err("trace must begin with SetGST".to_owned());
    }
    if steps
        .iter()
        .skip(1)
        .any(|step| step.action == ModelAction::SetGst)
    {
        return Err("SetGST may appear only once".to_owned());
    }

    let mut views = [0_u64; 4];
    let mut timeout_begun = BTreeSet::new();
    let mut timeout_persisted = BTreeSet::new();
    let mut timeout_signed = BTreeSet::new();
    let mut delivered_timeouts: BTreeMap<(usize, u64), BTreeSet<usize>> = BTreeMap::new();
    let mut formed_timeout_certificates = BTreeSet::new();
    let mut delivered_timeout_certificates = BTreeSet::new();
    let mut install_begun = BTreeSet::new();
    let mut assembled = BTreeSet::new();
    let mut proposal_begun = BTreeSet::new();
    let mut proposal_persisted = BTreeSet::new();
    let mut proposal_signed = BTreeSet::new();
    let mut proposal_delivered = BTreeSet::new();
    let mut fetched = BTreeSet::new();
    let mut stored = BTreeSet::new();
    let mut validated = BTreeSet::new();
    let mut prepare_begun = BTreeSet::new();
    let mut prepare_persisted = BTreeSet::new();
    let mut vote_signed = BTreeSet::new();
    let mut delivered_votes: BTreeMap<(usize, u64, Phase, ModelSubject), BTreeSet<usize>> =
        BTreeMap::new();
    let mut formed_quorum_certificates = BTreeSet::new();
    let mut delivered_quorum_certificates = BTreeSet::new();
    let mut observe_begun = BTreeSet::new();
    let mut lock_begun = BTreeSet::new();
    let mut lock_persisted = BTreeSet::new();
    let mut active_locks = [None; 4];
    let mut decisions_begun = BTreeSet::new();

    for step in steps.iter().copied().skip(1) {
        let error = |message: &str| format!("step {} {:?}: {message}", step.number, step.action);
        match step.action {
            ModelAction::SetGst => unreachable!("duplicate SetGST rejected above"),
            ModelAction::BeginTimeout => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                if views[node] != view || !timeout_begun.insert((node, view)) {
                    return Err(error("timeout is stale or duplicated"));
                }
            }
            ModelAction::PersistTimeout => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                );
                if !timeout_begun.contains(&key) || !timeout_persisted.insert(key) {
                    return Err(error("timeout persistence lacks one begin action"));
                }
            }
            ModelAction::CompleteTimeoutSignature => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                );
                if !timeout_persisted.contains(&key) || !timeout_signed.insert(key) {
                    return Err(error("timeout signature lacks a durable unique intent"));
                }
            }
            ModelAction::DeliverTimeout => {
                let node = required(step.node, step, "node")?;
                let peer = required(step.peer, step, "peer")?;
                let view = required(step.view, step, "view")?;
                if !timeout_signed.contains(&(peer, view)) {
                    return Err(error("timeout delivery has no signed source"));
                }
                delivered_timeouts
                    .entry((node, view))
                    .or_default()
                    .insert(peer);
            }
            ModelAction::FormTc => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                if delivered_timeouts
                    .get(&(node, view))
                    .map_or(0, BTreeSet::len)
                    < 3
                    || !formed_timeout_certificates.insert((node, view))
                {
                    return Err(error("TC formation lacks a distinct-validator quorum"));
                }
            }
            ModelAction::PersistInstallTc => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                if !formed_timeout_certificates.contains(&(node, view))
                    && !install_begun.contains(&(node, view))
                {
                    return Err(error(
                        "TC installation was neither locally formed nor begun",
                    ));
                }
                if views[node] > view {
                    return Err(error("TC installation regresses the view"));
                }
                views[node] = view + 1;
            }
            ModelAction::DeliverTc => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                if !formed_timeout_certificates
                    .iter()
                    .any(|(_, formed_view)| *formed_view == view)
                {
                    return Err(error("TC delivery has no formed certificate"));
                }
                delivered_timeout_certificates.insert((node, view));
            }
            ModelAction::BeginInstallTc => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                );
                if !delivered_timeout_certificates.contains(&key) || !install_begun.insert(key) {
                    return Err(error("TC installation lacks one delivery"));
                }
            }
            ModelAction::AssembleLocalBody => {
                let node = required(step.node, step, "node")?;
                let subject = required(step.subject, step, "subject")?;
                let view_offset = usize::try_from(views[node] % 4)
                    .map_err(|_| error("view offset does not fit usize"))?;
                if node != (3 + view_offset) % 4 || !assembled.insert((node, subject)) {
                    return Err(error(
                        "body was not assembled by the expected unique leader",
                    ));
                }
            }
            ModelAction::BeginLocalProposal => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let subject = required(step.subject, step, "subject")?;
                let view_offset = usize::try_from(view % 4)
                    .map_err(|_| error("view offset does not fit usize"))?;
                if views[node] != view
                    || node != (3 + view_offset) % 4
                    || !assembled.contains(&(node, subject))
                    || !proposal_begun.insert((node, view, subject))
                {
                    return Err(error(
                        "proposal violates leader, view, body, or uniqueness rules",
                    ));
                }
            }
            ModelAction::PersistProposal => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    required(step.subject, step, "subject")?,
                );
                if !proposal_begun.contains(&key) || !proposal_persisted.insert(key) {
                    return Err(error("proposal persistence lacks one begin action"));
                }
            }
            ModelAction::CompleteProposalSignature => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    required(step.subject, step, "subject")?,
                );
                if !proposal_persisted.contains(&key) || !proposal_signed.insert(key) {
                    return Err(error("proposal signature lacks a durable unique intent"));
                }
            }
            ModelAction::DeliverProposal => {
                let node = required(step.node, step, "node")?;
                let peer = required(step.peer, step, "peer")?;
                let view = required(step.view, step, "view")?;
                let subject = required(step.subject, step, "subject")?;
                if !proposal_signed.contains(&(peer, view, subject)) {
                    return Err(error("proposal delivery has no signed leader proposal"));
                }
                proposal_delivered.insert((node, view, subject));
            }
            ModelAction::FetchBody => {
                let node = required(step.node, step, "node")?;
                let subject = required(step.subject, step, "subject")?;
                if !proposal_delivered
                    .iter()
                    .any(|(recipient, _, proposal_subject)| {
                        *recipient == node && *proposal_subject == subject
                    })
                    || !fetched.insert((node, subject))
                {
                    return Err(error("body fetch lacks one delivered proposal"));
                }
            }
            ModelAction::StoreBody => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.subject, step, "subject")?,
                );
                if !fetched.contains(&key) || !stored.insert(key) {
                    return Err(error("body storage lacks one completed fetch"));
                }
            }
            ModelAction::ValidateBody => {
                let node = required(step.node, step, "node")?;
                let subject = required(step.subject, step, "subject")?;
                if !stored.contains(&(node, subject)) || !validated.insert((node, subject)) {
                    return Err(error("validation lacks one durably stored body"));
                }
            }
            ModelAction::BeginPrepare => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let subject = required(step.subject, step, "subject")?;
                let local_valid = assembled.contains(&(node, subject));
                if step.phase != Some(Phase::Prepare)
                    || (!local_valid && !validated.contains(&(node, subject)))
                    || !proposal_delivered.contains(&(node, view, subject))
                    || !prepare_begun.insert((node, view, subject))
                {
                    return Err(error(
                        "Prepare lacks proposal delivery and validated availability",
                    ));
                }
            }
            ModelAction::PersistPrepare => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    required(step.subject, step, "subject")?,
                );
                if step.phase != Some(Phase::Prepare)
                    || !prepare_begun.contains(&key)
                    || !prepare_persisted.insert(key)
                {
                    return Err(error("Prepare persistence lacks one begin action"));
                }
            }
            ModelAction::CompleteVoteSignature => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let phase = required(step.phase, step, "phase")?;
                let subject = required(step.subject, step, "subject")?;
                let authorized = match phase {
                    Phase::Prepare => prepare_persisted.contains(&(node, view, subject)),
                    Phase::Commit => lock_persisted.contains(&(node, view, subject)),
                };
                let receiver_admits = match phase {
                    Phase::Prepare => views[node] == view,
                    Phase::Commit => active_locks[node] == Some((view, subject)),
                };
                if !authorized
                    || !receiver_admits
                    || !vote_signed.insert((node, view, phase, subject))
                {
                    return Err(error("vote signature lacks a durable unique intent"));
                }
                delivered_votes
                    .entry((node, view, phase, subject))
                    .or_default()
                    .insert(node);
            }
            ModelAction::DeliverVote => {
                let node = required(step.node, step, "node")?;
                let peer = required(step.peer, step, "peer")?;
                let view = required(step.view, step, "view")?;
                let phase = required(step.phase, step, "phase")?;
                let subject = required(step.subject, step, "subject")?;
                if !vote_signed.contains(&(peer, view, phase, subject)) {
                    return Err(error("vote delivery has no durable signed source"));
                }
                let receiver_admits = match phase {
                    Phase::Prepare => views[node] == view,
                    Phase::Commit => active_locks[node] == Some((view, subject)),
                };
                if receiver_admits {
                    delivered_votes
                        .entry((node, view, phase, subject))
                        .or_default()
                        .insert(peer);
                }
            }
            ModelAction::FormPrepareQc | ModelAction::FormCommitQc => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let phase = required(step.phase, step, "phase")?;
                let subject = required(step.subject, step, "subject")?;
                let expected = if step.action == ModelAction::FormPrepareQc {
                    Phase::Prepare
                } else {
                    Phase::Commit
                };
                if phase != expected
                    || (phase == Phase::Commit && active_locks[node] != Some((view, subject)))
                    || delivered_votes
                        .get(&(node, view, phase, subject))
                        .map_or(0, BTreeSet::len)
                        < 3
                    || !formed_quorum_certificates.insert((node, view, phase, subject))
                {
                    return Err(error(
                        "QC formation lacks a distinct-validator phase quorum",
                    ));
                }
                if phase == Phase::Commit {
                    decisions_begun.insert((node, view, subject));
                }
            }
            ModelAction::DeliverQc => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let phase = required(step.phase, step, "phase")?;
                let subject = required(step.subject, step, "subject")?;
                if !formed_quorum_certificates.iter().any(
                    |(_, formed_view, formed_phase, formed_subject)| {
                        *formed_view == view && *formed_phase == phase && *formed_subject == subject
                    },
                ) {
                    return Err(error("QC delivery has no formed certificate"));
                }
                delivered_quorum_certificates.insert((node, view, phase, subject));
            }
            ModelAction::BeginObservePrepare => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    Phase::Prepare,
                    required(step.subject, step, "subject")?,
                );
                if step.phase != Some(Phase::Prepare)
                    || !delivered_quorum_certificates.contains(&key)
                    || !observe_begun.insert((key.0, key.1, key.3))
                {
                    return Err(error("PrepareQC observation lacks one delivery"));
                }
            }
            ModelAction::PersistObservePrepare => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    required(step.subject, step, "subject")?,
                );
                if !observe_begun.remove(&key) {
                    return Err(error("PrepareQC persistence lacks one observation"));
                }
            }
            ModelAction::BeginLockCommit => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let subject = required(step.subject, step, "subject")?;
                if !delivered_quorum_certificates.contains(&(node, view, Phase::Prepare, subject))
                    || !lock_begun.insert((node, view, subject))
                {
                    return Err(error("Commit lock lacks one delivered PrepareQC"));
                }
            }
            ModelAction::PersistLockCommit => {
                let node = required(step.node, step, "node")?;
                let view = required(step.view, step, "view")?;
                let subject = required(step.subject, step, "subject")?;
                let key = (node, view, subject);
                if !lock_begun.contains(&key) || !lock_persisted.insert(key) {
                    return Err(error("Commit persistence lacks one lock action"));
                }
                active_locks[node] = Some((view, subject));
                delivered_votes.retain(|(recipient, vote_view, phase, vote_subject), _| {
                    *recipient != node
                        || *vote_view == views[node]
                        || (*phase == Phase::Commit
                            && *vote_view == view
                            && *vote_subject == subject)
                });
            }
            ModelAction::PersistDecision => {
                let key = (
                    required(step.node, step, "node")?,
                    required(step.view, step, "view")?,
                    required(step.subject, step, "subject")?,
                );
                if step.phase != Some(Phase::Commit) || !decisions_begun.remove(&key) {
                    return Err(error("decision persistence lacks one formed CommitQC"));
                }
            }
        }
    }
    if !decisions_begun.is_empty() {
        return Err("trace ends with an unpersisted decision".to_owned());
    }
    Ok(())
}

#[derive(Clone)]
struct Envelope {
    from: usize,
    to: usize,
    message: ConsensusMessageV2,
}

enum DeferredEvent {
    AuthenticatedIngress(Event),
    Completion(Event),
}

struct ReplayNode {
    reducer: Reducer,
    wal: Vec<WalEntry>,
    pending: VecDeque<Effect>,
    deferred: VecDeque<DeferredEvent>,
    applied: Vec<Subject>,
}

struct ProductionReplay {
    context: HeightContext,
    nodes: Vec<ReplayNode>,
    network: Vec<Envelope>,
    assembled: BTreeSet<(usize, ModelSubject)>,
    signatures: u64,
    backpressured: usize,
    reordered_deliveries: usize,
    reports: Vec<(ValidatorId, EquivocationKind)>,
}

impl ProductionReplay {
    fn new() -> Self {
        let context = production_context();
        let nodes = context
            .roster()
            .iter()
            .map(|validator| ReplayNode {
                reducer: Reducer::new(context.clone(), Some(validator.id()), Generation::new(1))
                    .expect("fixture validator belongs to the frozen roster"),
                wal: Vec::new(),
                pending: VecDeque::new(),
                deferred: VecDeque::new(),
                applied: Vec::new(),
            })
            .collect();
        Self {
            context,
            nodes,
            network: Vec::new(),
            assembled: BTreeSet::new(),
            signatures: 0,
            backpressured: 0,
            reordered_deliveries: 0,
            reports: Vec::new(),
        }
    }

    fn dispatch(&mut self, node: usize, event: Event) -> StepDisposition {
        let outcome = self.nodes[node]
            .reducer
            .step(event)
            .unwrap_or_else(|error| panic!("production node {node} rejected trace input: {error}"));
        let disposition = outcome.disposition();
        self.absorb(node, outcome.into_effects());
        disposition
    }

    fn dispatch_deferred(&mut self, node: usize, deferred: DeferredEvent) {
        let event = match &deferred {
            DeferredEvent::AuthenticatedIngress(event) => event
                .clone()
                .retag_authenticated_ingress(self.nodes[node].reducer.current_tag()),
            DeferredEvent::Completion(event) => event.clone(),
        };
        let outcome = self.nodes[node]
            .reducer
            .step(event)
            .unwrap_or_else(|error| {
                panic!("production node {node} rejected deferred input: {error}")
            });
        if outcome.disposition() == StepDisposition::Ignored(IgnoreReason::Busy) {
            self.backpressured += 1;
            self.nodes[node].deferred.push_back(deferred);
        } else {
            self.absorb(node, outcome.into_effects());
        }
    }

    fn retry_deferred(&mut self, node: usize) {
        let attempts = self.nodes[node].deferred.len();
        for _ in 0..attempts {
            let deferred = self.nodes[node]
                .deferred
                .pop_front()
                .expect("bounded retry count came from queue length");
            self.dispatch_deferred(node, deferred);
            if !self.nodes[node].pending.is_empty() {
                break;
            }
        }
    }

    fn absorb(&mut self, from: usize, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::Broadcast(message) => {
                    for to in 0..self.nodes.len() {
                        self.network.push(Envelope {
                            from,
                            to,
                            message: message.clone(),
                        });
                    }
                }
                Effect::EnterView {
                    tag, certificate, ..
                } => {
                    assert_eq!(tag, self.nodes[from].reducer.current_tag());
                    assert_eq!(tag.view(), certificate.round().view() + 1);
                }
                Effect::ReportEquivocation { evidence } => {
                    self.reports.push((evidence.offender(), evidence.kind()));
                }
                Effect::ReportInvalidCertifiedBody { .. } => {
                    panic!("the valid TLC witness must not certify an invalid body")
                }
                effect => self.nodes[from].pending.push_back(effect),
            }
        }
    }

    fn pending_position(&self, node: usize, predicate: impl Fn(&Effect) -> bool) -> usize {
        self.nodes[node]
            .pending
            .iter()
            .position(predicate)
            .unwrap_or_else(|| {
                panic!(
                    "node {node} has no matching effect; pending={:?}",
                    self.nodes[node].pending
                )
            })
    }

    fn take_pending(&mut self, node: usize, predicate: impl Fn(&Effect) -> bool) -> Effect {
        let position = self.pending_position(node, predicate);
        self.nodes[node]
            .pending
            .remove(position)
            .expect("located pending effect remains present")
    }

    fn has_persist(&self, node: usize, predicate: impl Fn(&WalRecord) -> bool) -> bool {
        self.nodes[node].pending.iter().any(
            |effect| matches!(effect, Effect::Persist { entry, .. } if predicate(entry.record())),
        )
    }

    fn acknowledge_persist(&mut self, node: usize, predicate: impl Fn(&WalRecord) -> bool) {
        let Effect::Persist { tag, entry } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::Persist { entry, .. } if predicate(entry.record())),
        ) else {
            unreachable!("pending predicate selected a persistence effect")
        };
        self.nodes[node].wal.push(entry.clone());
        let disposition = self.dispatch(
            node,
            Event::Persisted {
                tag,
                id: entry.id(),
            },
        );
        assert_eq!(disposition, StepDisposition::Applied);
        self.retry_deferred(node);
    }

    fn complete_signature(&mut self, node: usize, predicate: impl Fn(&SignableMessage) -> bool) {
        let Effect::Sign { tag, message: _ } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::Sign { message, .. } if predicate(message)),
        ) else {
            unreachable!("pending predicate selected a signing effect")
        };
        self.signatures += 1;
        let signer = model_validator(node);
        let signature = OpaqueSignature::new(vec![
            signer.as_bytes()[0],
            u8::try_from(self.signatures & 0xff).expect("masked signature sequence fits u8"),
        ]);
        let disposition = self.dispatch(node, Event::Signed { tag, signature });
        assert_eq!(disposition, StepDisposition::Applied);
        self.retry_deferred(node);
    }

    fn complete_fetch(&mut self, node: usize, subject: Subject) {
        let Effect::FetchBody {
            tag,
            round,
            subject: fetched,
            ..
        } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::FetchBody { subject: value, .. } if *value == subject),
        ) else {
            unreachable!("pending predicate selected a fetch effect")
        };
        assert_eq!(fetched, subject);
        self.dispatch_deferred(
            node,
            DeferredEvent::Completion(Event::BodyAvailable {
                tag,
                round,
                subject,
            }),
        );
    }

    fn complete_store(&mut self, node: usize, subject: Subject) {
        let Effect::StoreBody {
            tag,
            round,
            subject: stored,
        } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::StoreBody { subject: value, .. } if *value == subject),
        ) else {
            unreachable!("pending predicate selected a body-store effect")
        };
        assert_eq!(stored, subject);
        self.dispatch_deferred(
            node,
            DeferredEvent::Completion(Event::BodyStored {
                tag,
                round,
                subject,
            }),
        );
    }

    fn complete_validation(&mut self, node: usize, subject: Subject, valid: bool) {
        let Effect::ValidateBody {
            tag,
            round,
            subject: validated,
        } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::ValidateBody { subject: value, .. } if *value == subject),
        ) else {
            unreachable!("pending predicate selected a validation effect")
        };
        assert_eq!(validated, subject);
        self.dispatch_deferred(
            node,
            DeferredEvent::Completion(Event::ValidationCompleted {
                tag,
                round,
                subject,
                valid,
            }),
        );
    }

    fn deliver(&mut self, action: ModelAction, node: usize, peer: Option<usize>, step: ModelStep) {
        let position = self
            .network
            .iter()
            .position(|envelope| {
                envelope.to == node
                    && peer.is_none_or(|expected| envelope.from == expected)
                    && message_matches(&envelope.message, action, peer, step)
            })
            .unwrap_or_else(|| {
                panic!(
                    "step {} cannot find {:?} from {peer:?} to node {node}; queued={}",
                    step.number,
                    action,
                    self.network.len()
                )
            });
        let envelope = self.network.swap_remove(position);
        if position != 0 {
            self.reordered_deliveries += 1;
        }
        let event = network_event(&envelope.message, self.nodes[node].reducer.current_tag());
        self.dispatch_deferred(node, DeferredEvent::AuthenticatedIngress(event));
    }

    #[allow(clippy::too_many_lines)]
    fn replay_step(&mut self, step: ModelStep) {
        let subject = step.subject.map(ModelSubject::production);
        match step.action {
            ModelAction::SetGst => {}
            ModelAction::DeliverTimeout
            | ModelAction::DeliverProposal
            | ModelAction::DeliverVote => {
                self.deliver(step.action, step.node.unwrap(), step.peer, step);
            }
            ModelAction::DeliverTc | ModelAction::DeliverQc => {
                self.deliver(step.action, step.node.unwrap(), None, step);
            }
            ModelAction::AssembleLocalBody => {
                self.assembled.insert((step.node.unwrap(), step.subject.unwrap()));
            }
            ModelAction::BeginTimeout => {
                let node = step.node.unwrap();
                let tag = self.nodes[node].reducer.current_tag();
                assert_eq!(tag.view(), step.view.unwrap());
                assert_eq!(
                    self.dispatch(node, Event::TimeoutElapsed { tag }),
                    StepDisposition::Applied
                );
            }
            ModelAction::PersistTimeout => self.acknowledge_persist(step.node.unwrap(), |record| {
                matches!(record, WalRecord::TimeoutIntent(vote) if vote.round().view() == step.view.unwrap())
            }),
            ModelAction::CompleteTimeoutSignature => {
                self.complete_signature(step.node.unwrap(), |message| {
                    matches!(message, SignableMessage::TimeoutVote(vote) if vote.round().view() == step.view.unwrap())
                });
            }
            ModelAction::FormTc => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::InstallTimeout(tc) if tc.round().view() == step.view.unwrap())
                }));
            }
            ModelAction::PersistInstallTc => {
                self.acknowledge_persist(step.node.unwrap(), |record| {
                    matches!(record, WalRecord::InstallTimeout(tc) if tc.round().view() == step.view.unwrap())
                });
            }
            ModelAction::BeginInstallTc => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::InstallTimeout(tc) if tc.round().view() == step.view.unwrap())
                }));
            }
            ModelAction::BeginLocalProposal => {
                let node = step.node.unwrap();
                assert!(self.assembled.contains(&(node, step.subject.unwrap())));
                let tag = self.nodes[node].reducer.current_tag();
                assert_eq!(tag.view(), step.view.unwrap());
                assert_eq!(
                    self.dispatch(
                        node,
                        Event::LocalProposalReady {
                            tag,
                            manifest: manifest(subject.unwrap()),
                        },
                    ),
                    StepDisposition::Applied
                );
            }
            ModelAction::PersistProposal => {
                self.acknowledge_persist(step.node.unwrap(), |record| {
                    matches!(record, WalRecord::ProposalIntent(proposal) if proposal.round().view() == step.view.unwrap() && proposal.manifest().subject() == subject.unwrap())
                });
            }
            ModelAction::CompleteProposalSignature => {
                self.complete_signature(step.node.unwrap(), |message| {
                    matches!(message, SignableMessage::Proposal(proposal) if proposal.round().view() == step.view.unwrap() && proposal.manifest().subject() == subject.unwrap())
                });
            }
            ModelAction::FetchBody => self.complete_fetch(step.node.unwrap(), subject.unwrap()),
            ModelAction::StoreBody => self.complete_store(step.node.unwrap(), subject.unwrap()),
            ModelAction::ValidateBody => {
                self.complete_validation(step.node.unwrap(), subject.unwrap(), true);
            }
            ModelAction::BeginPrepare => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::PrepareIntent(vote) if vote.round().view() == step.view.unwrap() && vote.subject() == subject.unwrap())
                }));
            }
            ModelAction::PersistPrepare => {
                self.acknowledge_persist(step.node.unwrap(), |record| {
                    matches!(record, WalRecord::PrepareIntent(vote) if vote.round().view() == step.view.unwrap() && vote.subject() == subject.unwrap())
                });
            }
            ModelAction::CompleteVoteSignature => {
                self.complete_signature(step.node.unwrap(), |message| {
                    matches!(message, SignableMessage::Vote(vote) if vote.round().view() == step.view.unwrap() && vote.phase() == step.phase.unwrap() && vote.subject() == subject.unwrap())
                });
            }
            ModelAction::FormPrepareQc => {
                let view = step.view.unwrap();
                let subject = subject.unwrap();
                assert!(self.network.iter().any(|envelope| {
                    matches!(&envelope.message, ConsensusMessageV2::QuorumCertificate(qc) if qc.phase() == Phase::Prepare && qc.round().view() == view && qc.subject() == subject)
                }));
            }
            ModelAction::BeginObservePrepare => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::ObservePrepare(qc) | WalRecord::LockAndCommit { prepare: qc, .. } if qc.round().view() == step.view.unwrap() && qc.subject() == subject.unwrap())
                }));
            }
            ModelAction::PersistObservePrepare => {
                let node = step.node.unwrap();
                if self.has_persist(node, |record| {
                    matches!(record, WalRecord::ObservePrepare(qc) if qc.round().view() == step.view.unwrap() && qc.subject() == subject.unwrap())
                }) {
                    self.acknowledge_persist(node, |record| {
                        matches!(record, WalRecord::ObservePrepare(qc) if qc.round().view() == step.view.unwrap() && qc.subject() == subject.unwrap())
                    });
                } else {
                    // Production atomically persists highest PrepareQC, lock,
                    // and Commit intent. The TLA model exposes observation and
                    // locking as two adjacent durable actions.
                    assert!(self.has_persist(node, |record| {
                        matches!(record, WalRecord::LockAndCommit { prepare, .. } if prepare.round().view() == step.view.unwrap() && prepare.subject() == subject.unwrap())
                    }));
                }
            }
            ModelAction::BeginLockCommit => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::LockAndCommit { prepare, .. } if prepare.round().view() == step.view.unwrap() && prepare.subject() == subject.unwrap())
                }));
            }
            ModelAction::PersistLockCommit => {
                self.acknowledge_persist(step.node.unwrap(), |record| {
                    matches!(record, WalRecord::LockAndCommit { prepare, .. } if prepare.round().view() == step.view.unwrap() && prepare.subject() == subject.unwrap())
                });
            }
            ModelAction::FormCommitQc => {
                let node = step.node.unwrap();
                assert!(self.has_persist(node, |record| {
                    matches!(record, WalRecord::Decision(qc) if qc.round().view() == step.view.unwrap() && qc.subject() == subject.unwrap())
                }));
            }
            ModelAction::PersistDecision => {
                self.acknowledge_persist(step.node.unwrap(), |record| {
                    matches!(record, WalRecord::Decision(qc) if qc.round().view() == step.view.unwrap() && qc.subject() == subject.unwrap())
                });
            }
        }
    }

    fn complete_apply(&mut self, node: usize, expected: Subject) {
        let Effect::Apply {
            tag,
            subject,
            certificate,
        } = self.take_pending(
            node,
            |effect| matches!(effect, Effect::Apply { subject, .. } if *subject == expected),
        )
        else {
            unreachable!("pending predicate selected an apply effect")
        };
        assert_eq!(subject, expected);
        assert_eq!(certificate.subject(), expected);
        self.nodes[node].applied.push(expected);
        assert_eq!(
            self.dispatch(node, Event::ApplicationCompleted { tag, subject }),
            StepDisposition::Applied
        );
    }

    fn crash_and_recover(&mut self, node: usize) -> EventTag {
        let old_tag = self.nodes[node].reducer.current_tag();
        let next_generation = Generation::new(old_tag.generation().get() + 1);
        let local = self.nodes[node]
            .reducer
            .local_validator()
            .expect("replay nodes are validators");
        let reducer = Reducer::recover(
            self.context.clone(),
            Some(local),
            next_generation,
            self.nodes[node].wal.clone(),
        )
        .expect("acknowledged complete WAL prefix recovers");
        self.nodes[node].reducer = reducer;
        self.nodes[node].pending.clear();
        self.nodes[node].deferred.clear();
        let tag = self.nodes[node].reducer.current_tag();
        assert_eq!(
            self.dispatch(node, Event::ResumeAfterReplay { tag }),
            StepDisposition::Applied
        );
        old_tag
    }
}

fn message_matches(
    message: &ConsensusMessageV2,
    action: ModelAction,
    peer: Option<usize>,
    step: ModelStep,
) -> bool {
    let subject = step.subject.map(ModelSubject::production);
    match (action, message) {
        (ModelAction::DeliverTimeout, ConsensusMessageV2::TimeoutVote(vote)) => {
            vote.vote().round().view() == step.view.unwrap()
                && peer.is_some_and(|peer| vote.vote().signer() == model_validator(peer))
        }
        (ModelAction::DeliverTc, ConsensusMessageV2::TimeoutCertificate(tc)) => {
            tc.round().view() == step.view.unwrap()
        }
        (ModelAction::DeliverProposal, ConsensusMessageV2::Proposal(proposal)) => {
            proposal.proposal().round().view() == step.view.unwrap()
                && proposal.proposal().manifest().subject() == subject.unwrap()
                && peer.is_some_and(|peer| proposal.proposal().proposer() == model_validator(peer))
        }
        (ModelAction::DeliverVote, ConsensusMessageV2::Vote(vote)) => {
            vote.vote().round().view() == step.view.unwrap()
                && vote.vote().phase() == step.phase.unwrap()
                && vote.vote().subject() == subject.unwrap()
                && peer.is_some_and(|peer| vote.vote().signer() == model_validator(peer))
        }
        (ModelAction::DeliverQc, ConsensusMessageV2::QuorumCertificate(qc)) => {
            qc.round().view() == step.view.unwrap()
                && qc.phase() == step.phase.unwrap()
                && qc.subject() == subject.unwrap()
        }
        _ => false,
    }
}

fn network_event(message: &ConsensusMessageV2, current: EventTag) -> Event {
    match message {
        ConsensusMessageV2::Proposal(proposal) => Event::ProposalReceived {
            tag: current,
            proposal: proposal.clone(),
        },
        ConsensusMessageV2::Vote(vote) => Event::VoteReceived {
            tag: current,
            vote: vote.clone(),
        },
        ConsensusMessageV2::QuorumCertificate(certificate) => Event::QuorumCertificateReceived {
            tag: current,
            certificate: certificate.clone(),
        },
        ConsensusMessageV2::TimeoutVote(vote) => Event::TimeoutVoteReceived {
            tag: current,
            vote: vote.clone(),
        },
        ConsensusMessageV2::TimeoutCertificate(certificate) => Event::TimeoutCertificateReceived {
            tag: current,
            certificate: certificate.clone(),
        },
        ConsensusMessageV2::BodyRequest(_) | ConsensusMessageV2::BodyChunk(_) => {
            panic!("transport payloads are outside the consensus model trace")
        }
    }
}

fn model_validator(node: usize) -> ValidatorId {
    ValidatorId::repeat(u8::try_from(node + 1).expect("four-node model index fits u8"))
}

fn production_context() -> HeightContext {
    let roster = (0..4)
        .map(|node| Validator::new(model_validator(node), VotingPower::new(1)))
        .collect();
    let mut seed = [0_u8; 32];
    seed[31] = 3;
    HeightContext::new(
        ContextId::repeat(0xc1),
        ChainId::repeat(0xc2),
        HEIGHT,
        None,
        0,
        roster,
        VotingMode::Permissioned,
        Digest::repeat(0xc3),
        Digest::repeat(0xc4),
        Digest::new(seed),
    )
    .expect("production trace context is valid")
}

fn manifest(subject: Subject) -> PayloadManifest {
    PayloadManifest::new(subject, Digest::repeat(0xd1), Digest::repeat(0xd2), 512, 4)
}

fn signatures(signers: &[usize]) -> Vec<SignatureShare> {
    signers
        .iter()
        .map(|node| {
            SignatureShare::new(
                model_validator(*node),
                OpaqueSignature::new(vec![u8::try_from(*node).unwrap(), 0xee]),
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum WalKind {
    Proposal,
    Prepare,
    ObservePrepare,
    LockCommit,
    Timeout,
    InstallTimeout,
    Decision,
}

const fn wal_kind(record: &WalRecord) -> WalKind {
    match record {
        WalRecord::ProposalIntent(_) => WalKind::Proposal,
        WalRecord::PrepareIntent(_) => WalKind::Prepare,
        WalRecord::ObservePrepare(_) => WalKind::ObservePrepare,
        WalRecord::LockAndCommit { .. } => WalKind::LockCommit,
        WalRecord::TimeoutIntent(_) => WalKind::Timeout,
        WalRecord::InstallTimeout(_) => WalKind::InstallTimeout,
        WalRecord::Decision(_) => WalKind::Decision,
    }
}

fn assert_every_witness_wal_prefix_recovers(replay: &ProductionReplay, subject: Subject) {
    let observed_kinds: BTreeSet<_> = replay
        .nodes
        .iter()
        .flat_map(|node| node.wal.iter())
        .map(|entry| wal_kind(entry.record()))
        .collect();
    assert_eq!(
        observed_kinds,
        BTreeSet::from([
            WalKind::Proposal,
            WalKind::Prepare,
            WalKind::ObservePrepare,
            WalKind::LockCommit,
            WalKind::Timeout,
            WalKind::InstallTimeout,
            WalKind::Decision,
        ]),
        "the TLC witness must cross every safety-WAL record class"
    );

    for (node_index, node) in replay.nodes.iter().enumerate() {
        let local = node
            .reducer
            .local_validator()
            .expect("production replay nodes are validators");
        for prefix_len in 0..=node.wal.len() {
            let generation = u64::try_from(node_index * 32 + prefix_len + 100)
                .expect("bounded model prefix index fits u64");
            let mut recovered = Reducer::recover(
                replay.context.clone(),
                Some(local),
                Generation::new(generation),
                node.wal[..prefix_len].iter().cloned(),
            )
            .unwrap_or_else(|error| {
                panic!("node {node_index} WAL prefix {prefix_len} failed replay: {error}")
            });
            assert!(
                recovered
                    .durable_state()
                    .decision()
                    .is_none_or(|decision| decision.subject() == subject)
            );
            let tag = recovered.current_tag();
            let resumed = recovered
                .step(Event::ResumeAfterReplay { tag })
                .expect("complete TLC-witness WAL prefix resumes safely");
            assert_eq!(resumed.disposition(), StepDisposition::Applied);
        }
    }
}

#[test]
fn tlc_liveness_witness_replays_against_the_production_reducer() {
    let steps = parse_trace(TRACE).expect("checked-in source-aligned trace is valid");
    assert_eq!(steps.len(), 95);
    let mut source_locks = [None; 4];
    for step in &steps {
        match step.action {
            ModelAction::PersistLockCommit => {
                source_locks[step.node.unwrap()] =
                    Some((step.view.unwrap(), step.subject.unwrap()));
            }
            ModelAction::DeliverVote => {
                assert_ne!(
                    step.node, step.peer,
                    "local signature reconstruction makes self delivery redundant"
                );
                if step.phase == Some(Phase::Commit) {
                    assert_eq!(
                        source_locks[step.node.unwrap()],
                        Some((step.view.unwrap(), step.subject.unwrap())),
                        "checked-in Commit delivery must have the receiver's exact durable lock"
                    );
                }
            }
            _ => {}
        }
    }
    let mut counts = BTreeMap::new();
    for step in &steps {
        *counts.entry(step.action).or_insert(0_usize) += 1;
    }
    for required_action in [
        ModelAction::BeginTimeout,
        ModelAction::PersistTimeout,
        ModelAction::CompleteTimeoutSignature,
        ModelAction::DeliverTimeout,
        ModelAction::FormTc,
        ModelAction::PersistInstallTc,
        ModelAction::BeginLocalProposal,
        ModelAction::PersistProposal,
        ModelAction::CompleteProposalSignature,
        ModelAction::FetchBody,
        ModelAction::StoreBody,
        ModelAction::ValidateBody,
        ModelAction::PersistPrepare,
        ModelAction::FormPrepareQc,
        ModelAction::PersistLockCommit,
        ModelAction::FormCommitQc,
        ModelAction::PersistDecision,
    ] {
        assert!(
            counts.contains_key(&required_action),
            "missing {required_action:?}"
        );
    }
    let decided = steps
        .last()
        .filter(|step| step.action == ModelAction::PersistDecision)
        .and_then(|step| step.node)
        .expect("validated witness ends with one persisted decision");

    let mut replay = ProductionReplay::new();
    assert_eq!(replay.context.leader(0), model_validator(3));
    assert_eq!(replay.context.leader(1), model_validator(0));
    for step in steps {
        replay.replay_step(step);
    }

    let subject = ModelSubject::A.production();
    assert_eq!(
        replay.nodes[decided]
            .reducer
            .durable_state()
            .decision()
            .map(iroha_sumeragi_core::QuorumCertificate::subject),
        Some(subject)
    );
    replay.complete_apply(decided, subject);
    assert_eq!(replay.nodes[decided].applied, vec![subject]);
    assert!(
        replay.backpressured > 0,
        "trace must exercise serialized backpressure"
    );
    assert!(
        replay.reordered_deliveries > 0,
        "TLC delivery order must differ from production enqueue order"
    );
    assert!(
        !replay.network.is_empty(),
        "the finite witness must leave safely losable duplicate/control traffic"
    );
    assert!(replay.nodes.iter().all(|node| {
        node.reducer
            .durable_state()
            .decision()
            .is_none_or(|decision| decision.subject() == subject)
    }));
    assert!(replay.nodes.iter().all(|node| {
        node.reducer.durable_state().current_view() == 1
            && node.wal.iter().any(|entry| {
                matches!(entry.record(), WalRecord::InstallTimeout(tc) if tc.round().view() == 0)
            })
    }));
    assert_every_witness_wal_prefix_recovers(&replay, subject);
}

#[test]
fn identical_commit_envelope_stutters_before_lock_and_is_admitted_after_persistence() {
    let recipient = 0;
    let signer = 1;
    let mut replay = ProductionReplay::new();
    let context = replay.context.clone();
    let round = Round::new(context.height(), 0);
    let subject = ModelSubject::A.production();
    let commit = SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Commit,
            subject,
            model_validator(signer),
        ),
        OpaqueSignature::new(vec![0x31, 0x41]),
    );
    let envelope = ConsensusMessageV2::Vote(commit.clone());

    assert_eq!(
        replay.dispatch(
            recipient,
            network_event(&envelope, replay.nodes[recipient].reducer.current_tag(),),
        ),
        StepDisposition::Ignored(IgnoreReason::IrrelevantView)
    );
    assert!(
        replay.nodes[recipient]
            .reducer
            .durable_state()
            .locked()
            .is_none()
    );

    let prepare = iroha_sumeragi_core::QuorumCertificate::new(
        CertificateRef::new(context.id(), round, Phase::Prepare, subject),
        signatures(&[0, 1, 2]),
    );
    assert_eq!(
        replay.dispatch(
            recipient,
            Event::QuorumCertificateReceived {
                tag: replay.nodes[recipient].reducer.current_tag(),
                certificate: prepare,
            },
        ),
        StepDisposition::Applied
    );
    replay.acknowledge_persist(recipient, |record| {
        matches!(record, WalRecord::ObservePrepare(qc) if qc.round() == round && qc.subject() == subject)
    });
    replay.complete_fetch(recipient, subject);
    replay.complete_store(recipient, subject);
    replay.complete_validation(recipient, subject, true);
    replay.acknowledge_persist(recipient, |record| {
        matches!(record, WalRecord::LockAndCommit { prepare, .. } if prepare.round() == round && prepare.subject() == subject)
    });

    let locked = replay.nodes[recipient]
        .reducer
        .durable_state()
        .locked()
        .expect("LockAndCommit acknowledgement installs the exact durable lock");
    assert_eq!(locked.round(), round);
    assert_eq!(locked.subject(), subject);
    replay.complete_signature(recipient, |message| {
        matches!(message, SignableMessage::Vote(vote) if vote.round() == round && vote.phase() == Phase::Commit && vote.subject() == subject)
    });
    assert_eq!(
        replay.dispatch(
            recipient,
            network_event(&envelope, replay.nodes[recipient].reducer.current_tag(),),
        ),
        StepDisposition::Applied
    );
    let additional_signer = 2;
    let vote = ConsensusMessageV2::Vote(SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Commit,
            subject,
            model_validator(additional_signer),
        ),
        OpaqueSignature::new(vec![0x31, u8::try_from(additional_signer).unwrap()]),
    ));
    assert_eq!(
        replay.dispatch(
            recipient,
            network_event(&vote, replay.nodes[recipient].reducer.current_tag(),),
        ),
        StepDisposition::Applied
    );
    assert!(replay.has_persist(recipient, |record| {
        matches!(record, WalRecord::Decision(qc) if qc.round() == round && qc.subject() == subject)
    }));
    assert_eq!(
        envelope,
        ConsensusMessageV2::Vote(commit),
        "both deliveries use the identical authenticated Commit envelope"
    );
}

#[test]
fn malformed_and_unsafe_normalized_traces_fail_closed() {
    let unknown = TRACE.replacen("SetGST", "InventSafety", 1);
    assert!(
        parse_trace(&unknown)
            .unwrap_err()
            .contains("unknown model action")
    );

    let non_contiguous = TRACE.replacen("2\tBeginTimeout", "7\tBeginTimeout", 1);
    assert!(
        parse_trace(&non_contiguous)
            .unwrap_err()
            .contains("non-contiguous")
    );

    let wrong_leader = TRACE.replacen("40\tBeginLocalProposal\t0", "40\tBeginLocalProposal\t1", 1);
    assert!(
        parse_trace(&wrong_leader)
            .unwrap_err()
            .contains("proposal violates leader")
    );

    // Replace the third distinct Prepare signer delivered to node zero with a
    // duplicate signer. The syntactic trace remains well formed but the model
    // validator refuses to manufacture a QC from two distinct validators.
    let under_quorum = TRACE.replacen(
        "71\tDeliverVote\t0\t2\t1\tPrepare\tA",
        "71\tDeliverVote\t0\t1\t1\tPrepare\tA",
        1,
    );
    assert!(
        parse_trace(&under_quorum)
            .unwrap_err()
            .contains("distinct-validator phase quorum")
    );

    // Complete node zero's durable Commit signature, then deliver it to node
    // two before node two's LockAndCommit acknowledgement. The authenticated
    // packet is a safe receiver-side stutter and must not be counted toward
    // the later CommitQC.
    let unlocked_commit = TRACE
        .replacen(
            "76\tBeginLockCommit\t2\t-\t1\tPrepare\tA\n\
         77\tBeginLockCommit\t0\t-\t1\tPrepare\tA\n\
         78\tPersistLockCommit\t2\t-\t1\tPrepare\tA\n\
         79\tCompleteVoteSignature\t2\t-\t1\tCommit\tA\n\
         80\tPersistLockCommit\t0\t-\t1\tPrepare\tA\n\
         81\tCompleteVoteSignature\t0\t-\t1\tCommit\tA\n\
         82\tDeliverQC\t1\t-\t1\tPrepare\tA\n\
         83\tDeliverVote\t0\t2\t1\tCommit\tA\n\
         84\tDeliverVote\t2\t0\t1\tCommit\tA",
            "76\tBeginLockCommit\t2\t-\t1\tPrepare\tA\n\
         77\tBeginLockCommit\t0\t-\t1\tPrepare\tA\n\
         78\tPersistLockCommit\t0\t-\t1\tPrepare\tA\n\
         79\tCompleteVoteSignature\t0\t-\t1\tCommit\tA\n\
         80\tDeliverVote\t2\t0\t1\tCommit\tA\n\
         81\tPersistLockCommit\t2\t-\t1\tPrepare\tA\n\
         82\tDeliverQC\t1\t-\t1\tPrepare\tA\n\
         83\tCompleteVoteSignature\t2\t-\t1\tCommit\tA\n\
         84\tDeliverVote\t0\t2\t1\tCommit\tA",
            1,
        )
        .replacen(
            "92\tFormCommitQC\t1\t-\t1\tCommit\tA",
            "92\tFormCommitQC\t2\t-\t1\tCommit\tA",
            1,
        )
        .replacen(
            "95\tPersistDecision\t1\t-\t1\tCommit\tA",
            "95\tPersistDecision\t2\t-\t1\tCommit\tA",
            1,
        );
    assert!(
        parse_trace(&unlocked_commit)
            .unwrap_err()
            .contains("distinct-validator phase quorum")
    );

    let missing_column = TRACE.replacen(
        "2\tBeginTimeout\t1\t-\t0\t-\t-",
        "2\tBeginTimeout\t1\t-\t0\t-",
        1,
    );
    assert!(
        parse_trace(&missing_column)
            .unwrap_err()
            .contains("expected 7")
    );
}

#[test]
fn crash_replay_rejects_stale_completion_and_resumes_exact_intent() {
    let mut replay = ProductionReplay::new();
    let leader = 3;
    let subject = ModelSubject::A.production();
    let tag = replay.nodes[leader].reducer.current_tag();
    replay.dispatch(
        leader,
        Event::LocalProposalReady {
            tag,
            manifest: manifest(subject),
        },
    );
    replay.acknowledge_persist(leader, |record| {
        matches!(record, WalRecord::ProposalIntent(proposal) if proposal.manifest().subject() == subject)
    });
    let Effect::Sign {
        tag: stale_tag,
        message: stale_message,
    } = replay.take_pending(leader, |effect| {
        matches!(
            effect,
            Effect::Sign {
                message: SignableMessage::Proposal(_),
                ..
            }
        )
    })
    else {
        unreachable!("proposal persistence emits one signing request")
    };
    assert_eq!(stale_tag, tag);
    let old_tag = replay.crash_and_recover(leader);
    assert_eq!(old_tag, tag);
    assert!(replay.nodes[leader].pending.iter().any(|effect| {
        matches!(effect, Effect::Sign { message, .. } if message == &stale_message)
    }));

    let stale = replay.nodes[leader]
        .reducer
        .step(Event::Signed {
            tag: stale_tag,
            signature: OpaqueSignature::new(vec![0xff]),
        })
        .expect("stale completion is a safe stutter");
    assert_eq!(
        stale.disposition(),
        StepDisposition::Ignored(IgnoreReason::StaleGeneration)
    );
    assert!(stale.effects().is_empty());

    replay.complete_signature(leader, |message| message == &stale_message);
    assert!(replay.network.iter().any(|envelope| {
        matches!(&envelope.message, ConsensusMessageV2::Proposal(proposal) if proposal.proposal().manifest().subject() == subject)
    }));
}

#[test]
fn unsafe_certificate_and_vote_equivocation_do_not_decide() {
    let context = production_context();
    let subject_a = ModelSubject::A.production();
    let subject_b = ModelSubject::B.production();
    let round = Round::new(context.height(), 0);
    let mut reducer = Reducer::new(
        context.clone(),
        Some(model_validator(0)),
        Generation::new(9),
    )
    .expect("fixture reducer");

    let duplicate_signer_qc = iroha_sumeragi_core::QuorumCertificate::new(
        CertificateRef::new(context.id(), round, Phase::Commit, subject_a),
        signatures(&[0, 0, 1]),
    );
    assert_eq!(
        reducer.step(Event::QuorumCertificateReceived {
            tag: reducer.current_tag(),
            certificate: duplicate_signer_qc,
        }),
        Err(ReducerError::Quorum(QuorumError::SignersNotStrictlyOrdered))
    );
    assert!(reducer.durable_state().decision().is_none());

    let vote_a = SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Prepare,
            subject_a,
            model_validator(1),
        ),
        OpaqueSignature::new(vec![1]),
    );
    let vote_b = SignedVote::new(
        Vote::new(
            context.id(),
            round,
            Phase::Prepare,
            subject_b,
            model_validator(1),
        ),
        OpaqueSignature::new(vec![2]),
    );
    reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: vote_a.clone(),
        })
        .expect("first authenticated vote is retained");
    let equivocation = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: vote_b,
        })
        .expect("conflicting authenticated vote is reported");
    assert!(matches!(
        equivocation.effects(),
        [Effect::ReportEquivocation { evidence }]
            if evidence.offender() == model_validator(1)
                && evidence.kind() == EquivocationKind::Vote
    ));
    let duplicate = reducer
        .step(Event::VoteReceived {
            tag: reducer.current_tag(),
            vote: vote_a,
        })
        .expect("duplicate vote is a safe stutter");
    assert_eq!(
        duplicate.disposition(),
        StepDisposition::Ignored(IgnoreReason::Duplicate)
    );
    assert!(reducer.durable_state().decision().is_none());
}

#[test]
fn invalid_body_never_authorizes_prepare_or_decision() {
    let context = production_context();
    let subject_a = ModelSubject::A.production();
    let round = Round::new(context.height(), 0);
    let leader = context.leader(0);
    let proposal = iroha_sumeragi_core::Proposal::new(
        context.id(),
        round,
        leader,
        manifest(subject_a),
        iroha_sumeragi_core::ProposalJustification::ParentCommit(None),
    );
    let mut validator = Reducer::new(context, Some(model_validator(0)), Generation::new(10))
        .expect("fixture reducer");
    let received = validator
        .step(Event::ProposalReceived {
            tag: validator.current_tag(),
            proposal: iroha_sumeragi_core::SignedProposal::new(
                proposal,
                OpaqueSignature::new(vec![0xaa]),
            ),
        })
        .expect("valid proposal begins body acquisition");
    let Effect::FetchBody {
        tag,
        round,
        subject,
        ..
    } = &received.effects()[0]
    else {
        panic!("proposal must fetch its body")
    };
    let available = validator
        .step(Event::BodyAvailable {
            tag: *tag,
            round: *round,
            subject: *subject,
        })
        .expect("body reconstruction completes");
    let Effect::StoreBody {
        tag,
        round,
        subject,
    } = &available.effects()[0]
    else {
        panic!("body must cross durable storage")
    };
    let stored = validator
        .step(Event::BodyStored {
            tag: *tag,
            round: *round,
            subject: *subject,
        })
        .expect("body storage completes");
    let Effect::ValidateBody {
        tag,
        round,
        subject,
    } = &stored.effects()[0]
    else {
        panic!("durable body must be validated")
    };
    let invalid = validator
        .step(Event::ValidationCompleted {
            tag: *tag,
            round: *round,
            subject: *subject,
            valid: false,
        })
        .expect("deterministic invalidity is an applied result");
    assert_eq!(invalid.disposition(), StepDisposition::Applied);
    assert!(invalid.effects().is_empty());
    assert_eq!(validator.body_state(*round, *subject), BodyState::Invalid);
    assert!(validator.durable_state().prepare_intent(*round).is_none());
    assert!(validator.durable_state().decision().is_none());
}

#[test]
fn overlapping_timeout_groups_are_rejected_transactionally() {
    let context = production_context();
    let round = Round::new(context.height(), 0);
    let mut validator = Reducer::new(context, Some(model_validator(0)), Generation::new(11))
        .expect("fixture reducer");
    let before = validator.clone();
    let malformed_tc = TimeoutCertificate::new(
        validator.context().id(),
        round,
        vec![
            TimeoutSignatureGroup::new(None, signatures(&[0, 1])),
            TimeoutSignatureGroup::new(None, signatures(&[1, 2])),
        ],
    );
    assert!(matches!(
        validator.step(Event::TimeoutCertificateReceived {
            tag: validator.current_tag(),
            certificate: malformed_tc,
        }),
        Err(ReducerError::Quorum(
            QuorumError::TimeoutGroupsNotStrictlyOrdered | QuorumError::OverlappingTimeoutSigner(_)
        ))
    ));
    assert_eq!(validator, before);
}

#[test]
fn timeout_equivocation_with_different_full_high_qcs_is_reported() {
    let context = production_context();
    let round = Round::new(context.height(), 0);
    let subject = ModelSubject::A.production();
    let prepare = iroha_sumeragi_core::QuorumCertificate::new(
        CertificateRef::new(context.id(), round, Phase::Prepare, subject),
        signatures(&[0, 1, 2]),
    );
    let signer = model_validator(3);
    let mut reducer = Reducer::new(
        context.clone(),
        Some(model_validator(0)),
        Generation::new(12),
    )
    .expect("fixture reducer");
    let no_high = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), round, signer, None),
        OpaqueSignature::new(vec![0x10]),
    );
    let with_high = SignedTimeoutVote::new(
        TimeoutVote::new(context.id(), round, signer, Some(prepare)),
        OpaqueSignature::new(vec![0x11]),
    );
    reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: no_high,
        })
        .expect("first timeout vote is retained");
    let outcome = reducer
        .step(Event::TimeoutVoteReceived {
            tag: reducer.current_tag(),
            vote: with_high,
        })
        .expect("conflicting timeout vote is reported");
    assert!(matches!(
        outcome.effects(),
        [Effect::ReportEquivocation { evidence }]
            if evidence.offender() == signer
                && evidence.kind() == EquivocationKind::Timeout
    ));
}
