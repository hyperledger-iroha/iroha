//! Serialized runtime shell for the authoritative Sumeragi v2 adapter.
//!
//! This module owns scheduling and backpressure, not consensus state. Every
//! admitted command is delivered to [`SumeragiV2Adapter`] by one serialized
//! class-aware arbiter, and all
//! returned [`AdapterEffect`] values are handed to callers unchanged. The only
//! effect inspected here is `EnterView`, because installing a certified view is
//! the sole event allowed to restart the round and retransmission clocks. The
//! round deadline grows linearly with the certified view while retransmission
//! retains its fixed base interval. This deterministic backoff eventually gives
//! a post-GST view enough time for bounded transport and durable body service.
//! A small deterministic arbiter gives the timeout priority while ensuring that
//! periodic retransmission cannot indefinitely exclude already-admitted work.
//! Completion, progress, and normal commands share one bounded allocation but
//! receive cyclic service, so a saturated normal prefix cannot starve a locked
//! Commit vote or a trusted local completion.

use std::{
    collections::VecDeque,
    fmt,
    time::{Duration, Instant},
};

use super::v2_core::{EventTag, ScheduleState, ScheduledWork};
use iroha_data_model::block::consensus_v2 as wire;

use super::{
    v2::{AdapterEffect, AdapterError, AuthenticatedConsensusMessage, SumeragiV2Adapter},
    v2_body_store::{DurableBodyReceipt, ValidatedBodyReceipt},
};

const RETRANSMIT_DIVISOR: u32 = 5;
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// Derive the deadline for one certified view from the immutable base timeout.
///
/// View zero receives the configured base timeout. Each later view adds one
/// more base interval, so any finite representable post-GST service bound is
/// eventually exceeded. Saturation avoids wraparound at the platform duration
/// limit; the protocol's liveness argument is conditioned on its finite bound
/// being representable by [`Duration`].
fn round_timeout_for_view(base_timeout: Duration, view: u64) -> Duration {
    let multiplier = u128::from(view) + 1;
    let total_nanos = base_timeout.as_nanos().saturating_mul(multiplier);
    let bounded_nanos = total_nanos.min(Duration::MAX.as_nanos());
    let seconds = u64::try_from(bounded_nanos / NANOS_PER_SECOND)
        .expect("duration nanoseconds were bounded by Duration::MAX");
    let nanoseconds = u32::try_from(bounded_nanos % NANOS_PER_SECOND)
        .expect("subsecond nanoseconds are below one billion");
    Duration::new(seconds, nanoseconds)
}

/// Capacity allocation for the single serialized command ingress.
///
/// Normal network traffic may use only the non-reserved prefix. Progress
/// messages (PrepareQCs, CommitQCs, and TCs) may additionally use the progress
/// reserve, and trusted asynchronous completions may use the whole queue. This
/// prevents an unbounded proposal/vote stream from excluding a CommitQC or a
/// completion while preserving FIFO order within each service class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueConfig {
    capacity: usize,
    progress_reserve: usize,
    completion_reserve: usize,
}

impl RuntimeQueueConfig {
    /// Construct a bounded class-aware ingress allocation.
    pub(crate) const fn new(
        capacity: usize,
        progress_reserve: usize,
        completion_reserve: usize,
    ) -> Self {
        Self {
            capacity,
            progress_reserve,
            completion_reserve,
        }
    }

    fn validate(self) -> Result<Self, RuntimeConfigError> {
        if self.capacity == 0
            || self.progress_reserve == 0
            || self.completion_reserve == 0
            || self
                .progress_reserve
                .checked_add(self.completion_reserve)
                .is_none_or(|reserved| reserved >= self.capacity)
        {
            return Err(RuntimeConfigError::InvalidQueueAllocation);
        }
        Ok(self)
    }

    const fn normal_limit(self) -> usize {
        self.capacity - self.progress_reserve - self.completion_reserve
    }

    const fn progress_limit(self) -> usize {
        self.capacity - self.completion_reserve
    }
}

impl Default for RuntimeQueueConfig {
    fn default() -> Self {
        Self::new(1024, 128, 256)
    }
}

/// Invalid immutable runtime configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeConfigError {
    /// The round timeout was zero or too small to derive a non-zero fifth.
    InvalidRoundTimeout,
    /// Queue capacity did not leave non-zero normal, progress, and completion
    /// allocations.
    InvalidQueueAllocation,
}

impl fmt::Display for RuntimeConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoundTimeout => formatter.write_str(
                "Sumeragi v2 round timeout must have a non-zero one-fifth retransmit interval",
            ),
            Self::InvalidQueueAllocation => formatter.write_str(
                "Sumeragi v2 runtime queue must reserve non-zero normal, progress, and completion capacity",
            ),
        }
    }
}

impl std::error::Error for RuntimeConfigError {}

/// Invalid activation of the live pacemaker clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeClockError {
    /// The one-shot post-startup activation already occurred.
    AlreadyArmed,
}

impl fmt::Display for RuntimeClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyArmed => formatter.write_str(
                "Sumeragi v2 live pacemaker clocks may be armed only once after startup",
            ),
        }
    }
}

impl std::error::Error for RuntimeClockError {}

/// Backpressure result from the bounded command ingress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnqueueError {
    /// Lower-priority traffic reached the boundary of capacity reserved for
    /// protocol progress or trusted completions.
    ReservedCapacity,
    /// The entire command ingress is full.
    Full,
    /// The runtime stopped accepting work after an adapter failure.
    FailClosed,
}

impl fmt::Display for EnqueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedCapacity => {
                formatter.write_str("Sumeragi v2 runtime reserved ingress capacity")
            }
            Self::Full => formatter.write_str("Sumeragi v2 runtime command ingress is full"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
        }
    }
}

impl std::error::Error for EnqueueError {}

/// Rejection while authenticating or admitting a network message.
#[derive(Debug)]
pub(crate) enum NetworkIngressError {
    /// Signature, structure, version, context, or canonical-manifest admission failed.
    Authentication(AdapterError),
    /// Payload belongs to the body/chunk transport rather than the reducer.
    TransportPayload,
    /// Authenticated input encountered bounded ingress backpressure.
    Backpressure(EnqueueError),
    /// The serialized runtime has already failed closed.
    FailClosed,
}

impl fmt::Display for NetworkIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authentication(error) => write!(formatter, "{error}"),
            Self::TransportPayload => formatter.write_str(
                "Sumeragi v2 transport payload must use the authenticated body transport",
            ),
            Self::Backpressure(error) => write!(formatter, "{error}"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
        }
    }
}

impl std::error::Error for NetworkIngressError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Authentication(error) => Some(error),
            Self::Backpressure(error) => Some(error),
            Self::TransportPayload | Self::FailClosed => None,
        }
    }
}

/// Fatal result from executing an already-admitted adapter input.
#[derive(Debug)]
pub(crate) enum RuntimeError<E> {
    /// The adapter rejected an admitted serialized transition.
    Driver(E),
    /// A previous driver failure permanently closed the runtime.
    FailClosed,
    /// The runner attempted live scheduling before startup finished.
    ClocksNotArmed,
    /// Interrupted-tip recovery was attempted after live scheduling began.
    RecoveryAfterClocksArmed,
}

impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(error) => write!(formatter, "Sumeragi v2 runtime failed closed: {error}"),
            Self::FailClosed => formatter.write_str("Sumeragi v2 runtime is fail-closed"),
            Self::ClocksNotArmed => {
                formatter.write_str("Sumeragi v2 pacemaker clocks are not armed")
            }
            Self::RecoveryAfterClocksArmed => formatter.write_str(
                "Sumeragi v2 interrupted-tip recovery cannot run after pacemaker clocks are armed",
            ),
        }
    }
}

impl<E> std::error::Error for RuntimeError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Driver(error) => Some(error),
            Self::FailClosed | Self::ClocksNotArmed | Self::RecoveryAfterClocksArmed => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandClass {
    Normal,
    Progress,
    Completion,
}

impl CommandClass {
    const fn next(self) -> Self {
        match self {
            Self::Completion => Self::Progress,
            Self::Progress => Self::Normal,
            Self::Normal => Self::Completion,
        }
    }
}

pub(crate) struct TaggedCommand<C> {
    tag: EventTag,
    class: CommandClass,
    command: C,
    admitted_at: Instant,
    eligible_skips: u64,
}

struct BoundedIngress<C> {
    config: RuntimeQueueConfig,
    commands: VecDeque<TaggedCommand<C>>,
    next_class: CommandClass,
}

impl<C> BoundedIngress<C> {
    fn new(config: RuntimeQueueConfig) -> Self {
        Self {
            config,
            commands: VecDeque::with_capacity(config.capacity),
            next_class: CommandClass::Completion,
        }
    }

    fn enqueue(&mut self, command: TaggedCommand<C>) -> Result<(), EnqueueError> {
        self.check_capacity(command.class)?;
        self.commands.push_back(command);
        Ok(())
    }

    fn check_capacity(&self, class: CommandClass) -> Result<(), EnqueueError> {
        let limit = match class {
            CommandClass::Normal => self.config.normal_limit(),
            CommandClass::Progress => self.config.progress_limit(),
            CommandClass::Completion => self.config.capacity,
        };
        if self.commands.len() >= limit {
            return Err(if self.commands.len() >= self.config.capacity {
                EnqueueError::Full
            } else {
                EnqueueError::ReservedCapacity
            });
        }
        Ok(())
    }

    fn pop_next(&mut self) -> Option<TaggedCommand<C>> {
        for _ in 0..3 {
            let class = self.next_class;
            self.next_class = self.next_class.next();
            let Some(index) = self
                .commands
                .iter()
                .position(|queued| queued.class == class)
            else {
                continue;
            };
            for skipped_class in [
                CommandClass::Completion,
                CommandClass::Progress,
                CommandClass::Normal,
            ] {
                if skipped_class == class {
                    continue;
                }
                if let Some(oldest) = self
                    .commands
                    .iter_mut()
                    .find(|queued| queued.class == skipped_class)
                {
                    oldest.eligible_skips = oldest.eligible_skips.saturating_add(1);
                }
            }
            return self.commands.remove(index);
        }
        None
    }

    fn len(&self) -> usize {
        self.commands.len()
    }

    fn remaining_capacity(&self) -> usize {
        self.config.capacity - self.commands.len()
    }

    fn lane_snapshot(&self, class: CommandClass, now: Instant) -> RuntimeQueueLaneSnapshot {
        let mut depth = 0usize;
        let mut oldest_age = None;
        let mut max_service_debt = 0u64;
        for queued in self.commands.iter().filter(|queued| queued.class == class) {
            depth = depth.saturating_add(1);
            let age = now.saturating_duration_since(queued.admitted_at);
            oldest_age = Some(oldest_age.map_or(age, |oldest: Duration| oldest.max(age)));
            max_service_debt = max_service_debt.max(queued.eligible_skips);
        }
        let capacity = match class {
            CommandClass::Normal => self.config.normal_limit(),
            CommandClass::Progress => self.config.progress_limit(),
            CommandClass::Completion => self.config.capacity,
        };
        RuntimeQueueLaneSnapshot {
            depth,
            capacity,
            oldest_age,
            max_service_debt,
        }
    }
}

/// Local operational snapshot for one serialized runtime lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueLaneSnapshot {
    /// Commands currently owned by the lane.
    pub(crate) depth: usize,
    /// Maximum total occupancy at which this class may still be admitted.
    pub(crate) capacity: usize,
    /// Age of the oldest command in this class.
    pub(crate) oldest_age: Option<Duration>,
    /// Eligible dispatches observed by the most-delayed queued command.
    pub(crate) max_service_debt: u64,
}

/// Local operational snapshot for all serialized runtime lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeQueueSnapshot {
    /// Ordinary proposal, vote, and timeout-vote work.
    pub(crate) normal: RuntimeQueueLaneSnapshot,
    /// Certified and exact-lock progress work.
    pub(crate) progress: RuntimeQueueLaneSnapshot,
    /// Trusted local I/O and application completions.
    pub(crate) completion: RuntimeQueueLaneSnapshot,
}

pub(crate) enum AdapterCommand {
    Authenticated(AuthenticatedConsensusMessage),
    LocalProposalReady {
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    },
    BodyAvailable {
        manifest: wire::PayloadManifest,
    },
    BodyStored {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    },
    ValidationSucceeded {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    },
    ValidationFailed {
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    },
    SignatureCompleted(Vec<u8>),
    ApplicationCompleted(wire::BlockSubject),
}

fn manifests_conflict_for_same_body(
    left: &wire::PayloadManifest,
    right: &wire::PayloadManifest,
) -> bool {
    left.round == right.round && left.subject == right.subject && left != right
}

impl AdapterCommand {
    fn is_same_authenticated_envelope(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        matches!(
            self,
            Self::Authenticated(queued)
                if queued.same_wire_envelope(authenticated)
        )
    }

    fn matches_wire_envelope(&self, message: &wire::ConsensusMessageV2) -> bool {
        matches!(
            self,
            Self::Authenticated(queued) if queued.matches_wire_envelope(message)
        )
    }

    fn matches_quorum_certificate(&self, certificate: &wire::QuorumCertificate) -> bool {
        matches!(
            self,
            Self::Authenticated(queued)
                if matches!(
                    queued.payload(),
                    wire::ConsensusMessageV2Payload::QuorumCertificate(queued)
                        if queued == certificate
                )
        )
    }

    fn is_authenticated_proposal_conflicting_with(
        &self,
        canonical: &wire::PayloadManifest,
    ) -> bool {
        let Self::Authenticated(message) = self else {
            return false;
        };
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload() else {
            return false;
        };
        manifests_conflict_for_same_body(&proposal.manifest, canonical)
    }
}

impl BoundedIngress<AdapterCommand> {
    fn authenticated_wire_tag(&self, message: &wire::ConsensusMessageV2) -> Option<EventTag> {
        self.commands.iter().find_map(|queued| {
            queued
                .command
                .matches_wire_envelope(message)
                .then_some(queued.tag)
        })
    }

    /// Check whether an independently authenticated form of `message` can
    /// either claim a new slot or coalesce with an exact queued owner.
    ///
    /// Raw equality is only a permission to spend authentication work while a
    /// prefix is saturated.  [`Self::enqueue_authenticated`] repeats equality
    /// on the resulting authenticated token before it coalesces anything.
    fn check_authenticated_wire_capacity(
        &self,
        message: &wire::ConsensusMessageV2,
        default_class: CommandClass,
        may_use_progress: bool,
    ) -> Result<(), EnqueueError> {
        if self.authenticated_wire_tag(message).is_some() {
            return Ok(());
        }
        match self.check_capacity(default_class) {
            Ok(()) => Ok(()),
            Err(_) if may_use_progress => self.check_capacity(CommandClass::Progress),
            Err(error) => Err(error),
        }
    }

    /// Check the reducer handoff performed after authenticating one block-sync
    /// response. The transport wrapper itself bypasses this ingress, but its
    /// embedded CommitQC must either claim Progress capacity or exactly
    /// coalesce with an authenticated QC already owned by the queue.
    fn check_embedded_quorum_certificate_capacity(
        &self,
        certificate: &wire::QuorumCertificate,
    ) -> Result<(), EnqueueError> {
        if self
            .commands
            .iter()
            .any(|queued| queued.command.matches_quorum_certificate(certificate))
        {
            return Ok(());
        }
        self.check_capacity(CommandClass::Progress)
    }

    /// Enqueue one independently authenticated envelope unless its exact wire
    /// value is already owned by this serialized queue.
    ///
    /// This is deliberately queue-scoped rather than height-long semantic
    /// suppression. Once the queued occurrence leaves, a later retransmission
    /// may be admitted and checked against the adapter's generation-aware
    /// delivery records in the usual way.
    fn enqueue_authenticated(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        authenticated: AuthenticatedConsensusMessage,
    ) -> Result<EventTag, EnqueueError> {
        if let Some(queued) = self.commands.iter().find(|queued| {
            queued
                .command
                .is_same_authenticated_envelope(&authenticated)
        }) {
            return Ok(queued.tag);
        }
        self.enqueue(TaggedCommand {
            tag,
            class,
            command: AdapterCommand::Authenticated(authenticated),
            admitted_at: Instant::now(),
            eligible_skips: 0,
        })?;
        Ok(tag)
    }

    fn enqueue_canonical_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        self.commands.retain(|queued| {
            !queued
                .command
                .is_authenticated_proposal_conflicting_with(&manifest)
        });
        self.enqueue(TaggedCommand {
            tag,
            class: CommandClass::Completion,
            command: AdapterCommand::BodyAvailable { manifest },
            admitted_at: Instant::now(),
            eligible_skips: 0,
        })
    }

    fn conflicts_with_pending_body_available(
        &self,
        authenticated: &AuthenticatedConsensusMessage,
    ) -> bool {
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = authenticated.payload() else {
            return false;
        };
        self.commands.iter().any(|queued| {
            let AdapterCommand::BodyAvailable { manifest } = &queued.command else {
                return false;
            };
            manifests_conflict_for_same_body(&proposal.manifest, manifest)
        })
    }
}

/// Minimal scheduling seam around the sole production adapter.
///
/// The generic parameter exists so clock and queue behavior can be tested
/// deterministically without constructing cryptographic contexts or a WAL.
/// Production uses the implementation for [`SumeragiV2Adapter`] below.
pub(crate) trait RuntimeDriver {
    /// Command payload consumed by the driver.
    type Command;
    /// Effect emitted unchanged to asynchronous adapters.
    type Effect;
    /// Fatal transition error.
    type Error;

    /// Current authoritative reducer tag.
    fn current_tag(&self) -> EventTag;
    /// Deliver one admitted command with its original tag.
    fn dispatch(
        &mut self,
        command: TaggedCommand<Self::Command>,
    ) -> Result<Vec<Self::Effect>, Self::Error>;
    /// Deliver the absolute round-timeout event.
    fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error>;
    /// Deliver one derived retransmission tick.
    fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error>;
    /// Identify only the effect which authorizes timer restart.
    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag>;
    /// Return whether the unauthenticated wire shape could match a protected
    /// active-lock item after authentication.
    fn wire_ingress_may_use_progress(&self, _payload: &wire::ConsensusMessageV2Payload) -> bool {
        false
    }
}

impl RuntimeDriver for SumeragiV2Adapter {
    type Command = AdapterCommand;
    type Effect = AdapterEffect;
    type Error = AdapterError;

    fn current_tag(&self) -> EventTag {
        SumeragiV2Adapter::current_tag(self)
    }

    fn dispatch(
        &mut self,
        tagged: TaggedCommand<Self::Command>,
    ) -> Result<Vec<Self::Effect>, Self::Error> {
        let tag = tagged.tag;
        let outcome = match tagged.command {
            AdapterCommand::Authenticated(message) => {
                // Authenticated network ingress is deliberately retagged by the
                // adapter if it waited behind a certified view transition.
                // Asynchronous completion variants below retain `tag` exactly.
                self.receive_authenticated(message)
            }
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            } => self.local_proposal_ready(tag, manifest, &durable_receipt, &validated_receipt),
            AdapterCommand::BodyAvailable { manifest } => self.body_available(tag, manifest),
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            } => self.body_stored(tag, round, subject, &receipt),
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            } => self.validation_succeeded(tag, round, subject, &receipt),
            AdapterCommand::ValidationFailed { round, subject } => {
                self.validation_failed(tag, round, subject)
            }
            AdapterCommand::SignatureCompleted(signature) => {
                self.signature_completed(tag, signature)
            }
            AdapterCommand::ApplicationCompleted(subject) => {
                self.application_completed(tag, subject)
            }
        }?;
        Ok(outcome.into_effects())
    }

    fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::timeout_elapsed(self, tag).map(|outcome| outcome.into_effects())
    }

    fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
        SumeragiV2Adapter::retransmit_elapsed(self, tag).map(|outcome| outcome.into_effects())
    }

    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
        match effect {
            AdapterEffect::EnterView { tag, .. } => Some(*tag),
            AdapterEffect::Sign { .. }
            | AdapterEffect::Broadcast(_)
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::ReportEquivocation { .. }
            | AdapterEffect::ReportInvalidCertifiedBody { .. } => None,
        }
    }

    fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
        SumeragiV2Adapter::wire_ingress_may_use_progress(self, payload)
    }
}

/// Result of one serialized scheduling step.
///
/// A step invokes the adapter at most once. Consequently, if that invocation
/// fails, no effects from a preceding invocation can be hidden by the error.
#[derive(Debug)]
pub(crate) enum RuntimeStep<E> {
    /// No timer was due and the command ingress was empty.
    Idle,
    /// One timer or command was delivered; effects remain in adapter order.
    Advanced(Vec<E>),
}

/// One-owner, class-aware scheduling shell for Sumeragi v2.
pub(crate) struct SerializedV2Runtime<D: RuntimeDriver = SumeragiV2Adapter> {
    driver: D,
    ingress: BoundedIngress<D::Command>,
    base_round_timeout: Duration,
    retransmit_interval: Duration,
    round_started_at: Instant,
    retransmit_started_at: Instant,
    round_tag: EventTag,
    clocks_armed: bool,
    timeout_emitted: bool,
    schedule: ScheduleState,
    fail_closed: bool,
}

impl<D: RuntimeDriver> SerializedV2Runtime<D> {
    fn with_driver(
        driver: D,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
        startup_effects: Vec<D::Effect>,
    ) -> Result<(Self, Vec<D::Effect>), RuntimeConfigError> {
        let retransmit_interval = round_timeout
            .checked_div(RETRANSMIT_DIVISOR)
            .filter(|interval| !interval.is_zero())
            .ok_or(RuntimeConfigError::InvalidRoundTimeout)?;
        let queue_config = queue_config.validate()?;
        let round_tag = driver.current_tag();
        let mut runtime = Self {
            driver,
            ingress: BoundedIngress::new(queue_config),
            base_round_timeout: round_timeout,
            retransmit_interval,
            round_started_at: started_at,
            retransmit_started_at: started_at,
            round_tag,
            clocks_armed: false,
            timeout_emitted: false,
            schedule: ScheduleState::default(),
            fail_closed: false,
        };
        runtime.observe_effects(started_at, &startup_effects);
        Ok((runtime, startup_effects))
    }

    fn enqueue(
        &mut self,
        tag: EventTag,
        class: CommandClass,
        command: D::Command,
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        self.ingress.enqueue(TaggedCommand {
            tag,
            class,
            command,
            admitted_at: Instant::now(),
            eligible_skips: 0,
        })
    }

    /// Run at most one timer or admitted command.
    ///
    /// Timeout wins when both clocks are due, and is emitted at most once for
    /// the installed view. A non-timeout timer may precede queued work once;
    /// the pure scheduler then owes admitted work the next slot. Retransmission
    /// runs at most once per call and advances from the actual service time,
    /// avoiding an unbounded catch-up burst after a paused process. Neither
    /// clock is changed by an arbitrary message or by any effect other than
    /// `EnterView`.
    pub(crate) fn step(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        if self.fail_closed {
            return Err(RuntimeError::FailClosed);
        }
        if !self.clocks_armed {
            return Err(RuntimeError::ClocksNotArmed);
        }

        let round_timeout = round_timeout_for_view(self.base_round_timeout, self.round_tag.view());
        let timeout_due = !self.timeout_emitted
            && now.saturating_duration_since(self.round_started_at) >= round_timeout;
        let retransmit_due =
            now.saturating_duration_since(self.retransmit_started_at) >= self.retransmit_interval;
        let (work, next_schedule) =
            self.schedule
                .select(timeout_due, retransmit_due, self.ingress.len() != 0);
        self.schedule = next_schedule;

        let effects = match work {
            ScheduledWork::Timeout => {
                self.timeout_emitted = true;
                match self.driver.timeout_elapsed(self.round_tag) {
                    Ok(effects) => effects,
                    Err(error) => return Err(self.close(error)),
                }
            }
            ScheduledWork::PeriodicTimer => {
                self.retransmit_started_at = now;
                match self.driver.retransmit_elapsed(self.round_tag) {
                    Ok(effects) => effects,
                    Err(error) => return Err(self.close(error)),
                }
            }
            ScheduledWork::Fifo => {
                let command = self
                    .ingress
                    .pop_next()
                    .expect("scheduler selected non-empty serialized ingress");
                match self.driver.dispatch(command) {
                    Ok(effects) => effects,
                    Err(error) => return Err(self.close(error)),
                }
            }
            ScheduledWork::Idle => return Ok(RuntimeStep::Idle),
        };
        self.observe_effects(now, &effects);
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Drain at most one startup-recovery command without running live timers.
    ///
    /// An interrupted canonical Kura tip is already decided and can require a
    /// slow local WSV/checkpoint/fsync replay before the height is retired. It
    /// must therefore keep the pacemaker unarmed: no peer can help this local
    /// operation, and elapsed wall time must not manufacture a timeout vote or
    /// retransmission. The runner consumes this runtime after finalization and
    /// constructs a fresh, normally armed successor-height runtime.
    pub(crate) fn step_recovery(
        &mut self,
        now: Instant,
    ) -> Result<RuntimeStep<D::Effect>, RuntimeError<D::Error>> {
        if self.fail_closed {
            return Err(RuntimeError::FailClosed);
        }
        if self.clocks_armed {
            return Err(RuntimeError::RecoveryAfterClocksArmed);
        }
        let Some(command) = self.ingress.pop_next() else {
            return Ok(RuntimeStep::Idle);
        };
        let effects = match self.driver.dispatch(command) {
            Ok(effects) => effects,
            Err(error) => return Err(self.close(error)),
        };
        self.observe_effects(now, &effects);
        Ok(RuntimeStep::Advanced(effects))
    }

    /// Number of admitted commands awaiting serialized delivery.
    pub(crate) fn queued_commands(&self) -> usize {
        self.ingress.len()
    }

    /// Per-class queue ownership, age, and service debt for diagnostics.
    pub(crate) fn queue_snapshot(&self, now: Instant) -> RuntimeQueueSnapshot {
        RuntimeQueueSnapshot {
            normal: self.ingress.lane_snapshot(CommandClass::Normal, now),
            progress: self.ingress.lane_snapshot(CommandClass::Progress, now),
            completion: self.ingress.lane_snapshot(CommandClass::Completion, now),
        }
    }

    /// View-aware diagnostic deadline for declaring a no-progress interval.
    ///
    /// The watchdog allows the complete current-view round deadline plus one
    /// fixed retransmission interval. Both values come from the configured
    /// pacemaker; saturation preserves a conservative diagnostic at the
    /// platform duration limit.
    pub(crate) fn watchdog_threshold(&self) -> Duration {
        round_timeout_for_view(self.base_round_timeout, self.round_tag.view())
            .checked_add(self.retransmit_interval)
            .unwrap_or(Duration::MAX)
    }

    /// Slots into which trusted asynchronous completions can be admitted now.
    ///
    /// Completion producers must consult this bound before removing work from
    /// their own bounded queues. Unlike normal and progress traffic,
    /// completions may use the entire ingress, so this is the exact free
    /// capacity.
    pub(crate) fn remaining_completion_capacity(&self) -> usize {
        self.ingress.remaining_capacity()
    }

    /// Return whether removing this network head can be coupled to immediate
    /// runtime admission.
    ///
    /// Reducer-directed traffic is checked against its exact Normal or
    /// Progress prefix in the single total-length ingress. Transport payloads
    /// do not enter this queue and therefore impose no runtime admission
    /// condition.
    #[cfg(test)]
    pub(crate) fn can_admit_network_payload(
        &self,
        payload: &wire::ConsensusMessageV2Payload,
    ) -> bool {
        let class = if self.driver.wire_ingress_may_use_progress(payload) {
            Some(CommandClass::Progress)
        } else {
            network_admission_class(payload)
        };
        class.is_none_or(|class| self.ingress.check_capacity(class).is_ok())
    }

    /// Tag of the view which owns the absolute clocks.
    pub(crate) const fn round_tag(&self) -> EventTag {
        self.round_tag
    }

    /// Arm the live clocks after all height constructors and startup effects.
    ///
    /// This one-shot boundary prevents WAL replay, body-store recovery, worker
    /// startup, and lane-work recovery from consuming the first live view's
    /// deadline. Once armed, only a certified `EnterView` effect may restart
    /// either clock.
    pub(crate) fn arm_live_clocks(&mut self, now: Instant) -> Result<(), RuntimeClockError> {
        if self.clocks_armed {
            return Err(RuntimeClockError::AlreadyArmed);
        }
        self.round_started_at = now;
        self.retransmit_started_at = now;
        self.timeout_emitted = false;
        self.schedule = ScheduleState::default();
        self.clocks_armed = true;
        Ok(())
    }

    /// View-indexed deadline currently owned by the runtime clock.
    #[cfg(test)]
    pub(crate) fn round_timeout(&self) -> Duration {
        round_timeout_for_view(self.base_round_timeout, self.round_tag.view())
    }

    /// Constant retransmission interval derived from the configured timeout.
    #[cfg(test)]
    pub(crate) const fn retransmit_interval(&self) -> Duration {
        self.retransmit_interval
    }

    /// Borrow the sole reducer driver without transferring ownership.
    pub(crate) const fn driver(&self) -> &D {
        &self.driver
    }

    /// Consume the shell and recover ownership of the adapter.
    pub(crate) fn into_driver(self) -> D {
        self.driver
    }

    fn observe_effects(&mut self, now: Instant, effects: &[D::Effect]) {
        for effect in effects {
            if let Some(tag) = D::enter_view_tag(effect) {
                self.round_tag = tag;
                self.round_started_at = now;
                self.retransmit_started_at = now;
                self.timeout_emitted = false;
                self.schedule = ScheduleState::default();
            }
        }
    }

    fn close(&mut self, error: D::Error) -> RuntimeError<D::Error> {
        self.fail_closed = true;
        RuntimeError::Driver(error)
    }
}

impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Take exclusive ownership of an opened adapter and preserve its recovery
    /// effects for immediate asynchronous dispatch.
    pub(crate) fn new(
        adapter: SumeragiV2Adapter,
        startup_effects: Vec<AdapterEffect>,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
    ) -> Result<(Self, Vec<AdapterEffect>), RuntimeConfigError> {
        Self::with_driver(
            adapter,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
        )
    }

    /// Read the reducer-owned proposal constraint without exposing mutable
    /// access to the authoritative adapter.
    pub(crate) fn local_proposal_directive(
        &self,
    ) -> Result<super::v2::LocalProposalDirective, AdapterError> {
        self.driver.local_proposal_directive()
    }

    /// Return the exact Decision key reconstructed by safety-WAL replay.
    pub(crate) fn replayed_decision_key(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        AdapterError,
    > {
        self.driver.replayed_decision_key()
    }

    /// Authenticate and enqueue one reducer-directed network message.
    ///
    /// Traffic which passes the bounded capacity check, or exactly matches an
    /// already-owned authenticated envelope, is cryptographically
    /// authenticated and then checked against canonical manifest authority.
    /// Rejections do not poison the runtime. Once admitted, any adapter
    /// transition failure is fatal when the serialized command is executed.
    pub(crate) fn enqueue_network(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<EventTag, NetworkIngressError> {
        let default_class = classify_reducer_network_ingress(self.fail_closed, &message.payload)?;
        // An exact queued retransmission may always spend authentication work
        // so it can release its ingress occurrence. Otherwise, only the
        // adapter's exact active-lock match may proceed after the normal prefix
        // fills. The authenticated predicate below remains the authority for
        // assigning the Progress class and for queue coalescing.
        let may_be_exact_locked_commit =
            self.driver.wire_ingress_may_use_progress(&message.payload);
        self.ingress
            .check_authenticated_wire_capacity(&message, default_class, may_be_exact_locked_commit)
            .map_err(NetworkIngressError::Backpressure)?;
        let authenticated = match self.driver.authenticate(message) {
            Ok(authenticated) => authenticated,
            Err(AdapterError::FailClosed | AdapterError::ReplayNotComplete) => {
                self.fail_closed = true;
                return Err(NetworkIngressError::FailClosed);
            }
            Err(error) => return Err(NetworkIngressError::Authentication(error)),
        };
        let class = if self
            .driver
            .authenticated_ingress_is_progress(&authenticated)
        {
            CommandClass::Progress
        } else {
            default_class
        };
        if self
            .ingress
            .conflicts_with_pending_body_available(&authenticated)
        {
            return Err(NetworkIngressError::Authentication(
                AdapterError::ConflictingManifest,
            ));
        }
        let tag = self.driver.current_tag();
        self.ingress
            .enqueue_authenticated(tag, class, authenticated)
            .map_err(NetworkIngressError::Backpressure)
    }

    /// Return whether the fair-ingress head can reach authentication and then
    /// either claim its exact runtime prefix or coalesce with an exact queued
    /// authenticated owner.
    pub(crate) fn can_admit_network_message(&self, message: &wire::ConsensusMessageV2) -> bool {
        if let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) =
            &message.payload
        {
            return !self.fail_closed
                && self
                    .ingress
                    .check_embedded_quorum_certificate_capacity(&response.certificate)
                    .is_ok();
        }
        let Some(default_class) = network_command_class(&message.payload) else {
            // Body/chunk transport does not enter the reducer FIFO.
            return true;
        };
        if self.fail_closed {
            return false;
        }
        let may_be_exact_locked_commit =
            self.driver.wire_ingress_may_use_progress(&message.payload);
        self.ingress
            .check_authenticated_wire_capacity(message, default_class, may_be_exact_locked_commit)
            .is_ok()
    }

    /// Enqueue a completed local proposal build with its original reducer tag.
    pub(crate) fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            },
        )
    }

    /// Enqueue successful canonical reconstruction with the exact fetch tag.
    ///
    /// Authenticated proposals already waiting in the FIFO are discarded only
    /// when they advertise a different manifest for this exact round and
    /// subject. Every retained command keeps its original relative order, and
    /// the completion is appended normally.
    pub(crate) fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        self.ingress.enqueue_canonical_body_available(tag, manifest)
    }

    /// Enqueue the durable body-store acknowledgement with its exact tag.
    pub(crate) fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            },
        )
    }

    /// Enqueue successful deterministic validation with its non-forgeable
    /// receipt and the tag of its currently attached reducer consumer.
    pub(crate) fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            },
        )
    }

    /// Enqueue deterministic validation rejection for its currently attached
    /// reducer consumer.
    pub(crate) fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::ValidationFailed { round, subject },
        )
    }

    /// Enqueue a signer completion without retagging it to the current view.
    pub(crate) fn enqueue_signature(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(signature),
        )
    }

    /// Enqueue an application completion without retagging it.
    pub(crate) fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(subject),
        )
    }
}

fn network_command_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => Some(CommandClass::Progress),
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_) => Some(CommandClass::Normal),
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => None,
    }
}

fn classify_reducer_network_ingress(
    fail_closed: bool,
    payload: &wire::ConsensusMessageV2Payload,
) -> Result<CommandClass, NetworkIngressError> {
    if fail_closed {
        return Err(NetworkIngressError::FailClosed);
    }
    network_command_class(payload).ok_or(NetworkIngressError::TransportPayload)
}

#[cfg(test)]
fn network_admission_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        // The transport wrapper is authenticated against an outstanding
        // request, then unwrapped into the embedded CommitQC and admitted to
        // the same Progress prefix before discovery state is retired.
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
            Some(CommandClass::Progress)
        }
        _ => network_command_class(payload),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use crate::sumeragi::v2_core::Generation;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::peer::PeerId;
    use tempfile::TempDir;

    use super::*;
    use crate::sumeragi::v2::{AdapterFingerprints, VerifiedHeightContext};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeCommand {
        record: Option<u8>,
        enter_view: Option<EventTag>,
        fail: bool,
    }

    impl FakeCommand {
        const fn record(value: u8) -> Self {
            Self {
                record: Some(value),
                enter_view: None,
                fail: false,
            }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                record: None,
                enter_view: Some(tag),
                fail: false,
            }
        }

        const fn fail() -> Self {
            Self {
                record: None,
                enter_view: None,
                fail: true,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeEffect {
        enter_view: Option<EventTag>,
    }

    impl FakeEffect {
        const fn other() -> Self {
            Self { enter_view: None }
        }

        const fn enter_view(tag: EventTag) -> Self {
            Self {
                enter_view: Some(tag),
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeError;

    impl fmt::Display for FakeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("fake driver failure")
        }
    }

    impl std::error::Error for FakeError {}

    struct FakeDriver {
        current_tag: EventTag,
        delivered: Vec<(EventTag, u8)>,
        timeouts: Vec<EventTag>,
        retransmits: Vec<EventTag>,
        timer_effects: VecDeque<Vec<FakeEffect>>,
        protected_commit: Option<(
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
    }

    impl FakeDriver {
        fn new(tag: EventTag) -> Self {
            Self {
                current_tag: tag,
                delivered: Vec::new(),
                timeouts: Vec::new(),
                retransmits: Vec::new(),
                timer_effects: VecDeque::new(),
                protected_commit: None,
            }
        }
    }

    impl RuntimeDriver for FakeDriver {
        type Command = FakeCommand;
        type Effect = FakeEffect;
        type Error = FakeError;

        fn current_tag(&self) -> EventTag {
            self.current_tag
        }

        fn dispatch(
            &mut self,
            tagged: TaggedCommand<Self::Command>,
        ) -> Result<Vec<Self::Effect>, Self::Error> {
            if tagged.command.fail {
                return Err(FakeError);
            }
            if let Some(tag) = tagged.command.enter_view {
                self.current_tag = tag;
                return Ok(vec![FakeEffect::enter_view(tag)]);
            }
            let value = tagged.command.record.expect("well-formed fake command");
            self.delivered.push((tagged.tag, value));
            Ok(vec![FakeEffect::other()])
        }

        fn timeout_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
            self.timeouts.push(tag);
            Ok(self.timer_effects.pop_front().unwrap_or_default())
        }

        fn retransmit_elapsed(&mut self, tag: EventTag) -> Result<Vec<Self::Effect>, Self::Error> {
            self.retransmits.push(tag);
            Ok(self.timer_effects.pop_front().unwrap_or_default())
        }

        fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag> {
            effect.enter_view
        }

        fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool {
            matches!(
                (payload, self.protected_commit),
                (
                    wire::ConsensusMessageV2Payload::Vote(vote),
                    Some((round, subject, execution_commitment))
                ) if vote.phase == wire::GlobalPhase::Commit
                    && vote.round == round
                    && vote.subject == subject
                    && vote.execution_commitment == execution_commitment
            )
        }
    }

    fn tag(view: u64) -> EventTag {
        EventTag::new(7, view, Generation::new(view + 11))
    }

    fn authenticated_proposal_for_test(
        manifest: wire::PayloadManifest,
    ) -> AuthenticatedConsensusMessage {
        AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(wire::Proposal {
                round: manifest.round,
                proposer: 0,
                subject: manifest.subject,
                manifest,
                justification: wire::ProposalJustification::ParentCommit(
                    wire::ParentCommitJustification { certificate: None },
                ),
                signature: vec![1],
            }),
        ))
    }

    fn authenticated_runtime_context() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic runtime ingress key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id: "sumeragi-v2-runtime-ingress-test".into(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("runtime fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"runtime ingress nexus context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024 * 1024,
                max_chunk_count: 1024,
            },
            leader_seed: [0x5A; 32],
        };
        (context, keys)
    }

    fn signed_runtime_proposal(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        marker: u8,
    ) -> wire::ConsensusMessageV2 {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new([marker, 1])),
            payload_hash: Hash::new([marker, 2]),
        };
        let body = vec![marker; 4];
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("small runtime fixture body"),
            &[body],
        )
        .expect("valid runtime fixture manifest");
        let proposer = context.leader(round.view);
        let mut proposal = wire::Proposal {
            round,
            proposer,
            subject,
            manifest,
            justification: wire::ProposalJustification::ParentCommit(
                wire::ParentCommitJustification { certificate: None },
            ),
            signature: Vec::new(),
        };
        proposal.signature = Signature::new(
            keys[usize::try_from(proposer).expect("small proposer index")].private_key(),
            &proposal.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal))
    }

    fn authenticated_network_runtime(
        directory: &TempDir,
        queue: RuntimeQueueConfig,
    ) -> (
        SerializedV2Runtime<SumeragiV2Adapter>,
        wire::HeightContext,
        Vec<KeyPair>,
    ) {
        let (context, keys) = authenticated_runtime_context();
        let proofs = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("runtime fixture proof of possession")
            })
            .collect();
        let verified =
            VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified fixture");
        let (adapter, startup) = SumeragiV2Adapter::open(
            directory.path().join("runtime-ingress-safety.wal"),
            verified,
            None,
            Generation::new(1),
            [0x31; 32],
            AdapterFingerprints {
                node: Hash::new(b"runtime ingress node"),
                build: Hash::new(b"runtime ingress build"),
                config: Hash::new(b"runtime ingress config"),
            },
        )
        .expect("open authenticated network runtime adapter");
        assert!(startup.is_empty());
        let runtime = SerializedV2Runtime::new(
            adapter,
            startup,
            Instant::now(),
            Duration::from_secs(10),
            queue,
        )
        .expect("valid authenticated network runtime")
        .0;
        (runtime, context, keys)
    }

    fn runtime(
        driver: FakeDriver,
        start: Instant,
        queue: RuntimeQueueConfig,
    ) -> SerializedV2Runtime<FakeDriver> {
        let mut runtime = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            queue,
            Vec::new(),
        )
        .expect("valid fake runtime")
        .0;
        runtime
            .arm_live_clocks(start)
            .expect("arm fake runtime after startup");
        runtime
    }

    fn enqueue_fake(
        runtime: &mut SerializedV2Runtime<FakeDriver>,
        tag: EventTag,
        class: CommandClass,
        command: FakeCommand,
    ) -> Result<(), EnqueueError> {
        runtime.enqueue(tag, class, command)
    }

    #[test]
    fn absolute_timeout_fires_once_and_messages_never_reset_it() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 8);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(10));
        assert_eq!(runtime.retransmit_interval(), Duration::from_secs(2));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(12));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("enqueue message");
        assert!(matches!(
            runtime.step(start + Duration::from_secs(1)),
            Ok(RuntimeStep::Advanced(_))
        ));

        assert!(matches!(
            runtime.step(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.retransmits, vec![initial]);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .expect("enqueue second message");
        let _ = runtime.step(start + Duration::from_secs(9));
        let _ = runtime.step(start + Duration::from_secs(10));
        let _ = runtime.step(start + Duration::from_secs(20));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
    }

    #[test]
    fn round_timeout_grows_linearly_by_view_without_wrapping() {
        let base = Duration::from_secs(10);
        assert_eq!(round_timeout_for_view(base, 0), base);
        assert_eq!(round_timeout_for_view(base, 1), Duration::from_secs(20));
        assert_eq!(round_timeout_for_view(base, 7), Duration::from_secs(80));
        assert_eq!(
            round_timeout_for_view(Duration::new(1, 500_000_000), 1),
            Duration::from_secs(3),
        );

        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX - 1),
            Duration::from_secs(u64::MAX)
        );
        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX),
            Duration::MAX
        );
        assert_eq!(round_timeout_for_view(Duration::MAX, 1), Duration::MAX);
    }

    #[test]
    fn recovered_nonzero_view_uses_scaled_timeout_from_live_arm() {
        let constructed_at = Instant::now();
        let armed_at = constructed_at + Duration::from_secs(500);
        let recovered = tag(4);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(recovered),
            constructed_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open recovered runtime");

        runtime
            .arm_live_clocks(armed_at)
            .expect("arm after recovered startup");
        assert_eq!(runtime.round_timeout(), Duration::from_secs(50));
        let _ = runtime.step(armed_at + Duration::from_secs(49));
        assert!(runtime.driver.timeouts.is_empty());
        let _ = runtime.step(armed_at + Duration::from_secs(50));
        assert_eq!(runtime.driver.timeouts, vec![recovered]);
    }

    #[test]
    fn class_aware_ingress_is_bounded_and_reserves_progress_and_completion_slots() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 4);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 3);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 2);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(99)
            ),
            Err(EnqueueError::ReservedCapacity)
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("reserved progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(4),
        )
        .expect("reserved completion slot");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Completion,
                FakeCommand::record(5)
            ),
            Err(EnqueueError::Full)
        );

        for offset in 0..4 {
            let _ = runtime.step(start + Duration::from_millis(offset));
        }
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 4), (initial, 3), (initial, 1), (initial, 2)]
        );
    }

    #[test]
    fn class_cursor_advances_from_the_served_class_after_empty_classes() {
        let admitted_at = Instant::now();
        let initial = tag(0);
        let queued = |class, value| TaggedCommand {
            tag: initial,
            class,
            command: FakeCommand::record(value),
            admitted_at,
            eligible_skips: 0,
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));

        ingress
            .enqueue(queued(CommandClass::Normal, 1))
            .expect("normal command fits the bounded ingress");
        let first = ingress.pop_next().expect("normal class is reachable");
        assert_eq!(first.command.record, Some(1));
        assert_eq!(ingress.next_class, CommandClass::Completion);

        ingress
            .enqueue(queued(CommandClass::Normal, 2))
            .expect("second normal command fits the bounded ingress");
        ingress
            .enqueue(queued(CommandClass::Completion, 3))
            .expect("completion reserve remains available");
        let second = ingress.pop_next().expect("completion class is selected");
        assert_eq!(second.command.record, Some(3));
        assert_eq!(ingress.next_class, CommandClass::Progress);

        let third = ingress
            .pop_next()
            .expect("empty progress class is skipped to normal");
        assert_eq!(third.command.record, Some(2));
        assert_eq!(ingress.next_class, CommandClass::Completion);
    }

    #[test]
    fn healthy_same_class_fifo_depth_does_not_accrue_service_debt() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        for id in 0..4 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(id),
            )
            .expect("enqueue same-class work");
        }

        let _ = runtime.step(start);
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.max_service_debt, 0);
    }

    #[test]
    fn canonical_body_completion_prunes_only_conflicting_queued_proposals() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"queued-body-context",
            ))),
            height: 7,
            view: 2,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"queued-body-block")),
            payload_hash: Hash::new(b"queued-body-payload"),
        };
        let layout = wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::Plain,
            chunk_size_bytes: 1,
            data_shards: 0,
            parity_shards: 0,
            max_payload_size_bytes: 1,
            max_chunk_count: 1,
        };
        let canonical = wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout,
            chunk_hashes: vec![Hash::new(b"canonical chunk")],
            chunk_root: Hash::new(b"canonical root"),
        };
        let conflicting = wire::PayloadManifest {
            chunk_hashes: vec![Hash::new(b"conflicting chunk")],
            chunk_root: Hash::new(b"conflicting root"),
            ..canonical.clone()
        };
        let other_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"other queued block")),
            payload_hash: Hash::new(b"other queued payload"),
            ..subject
        };
        let other = wire::PayloadManifest {
            subject: other_subject,
            ..conflicting.clone()
        };

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(8, 1, 1));
        for (command_tag, manifest) in [
            (tag(0), conflicting.clone()),
            (tag(1), canonical.clone()),
            (tag(2), other.clone()),
        ] {
            ingress
                .enqueue(TaggedCommand {
                    tag: command_tag,
                    class: CommandClass::Normal,
                    command: AdapterCommand::Authenticated(authenticated_proposal_for_test(
                        manifest,
                    )),
                    admitted_at: Instant::now(),
                    eligible_skips: 0,
                })
                .expect("queue authenticated proposal");
        }

        ingress
            .enqueue_canonical_body_available(tag(3), canonical.clone())
            .expect("trusted completion prunes its conflicting proposal and appends in FIFO order");
        assert_eq!(ingress.len(), 3);
        assert!(
            ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(
                conflicting
            ))
        );
        assert!(
            !ingress
                .conflicts_with_pending_body_available(&authenticated_proposal_for_test(canonical))
        );
        assert!(
            !ingress.conflicts_with_pending_body_available(&authenticated_proposal_for_test(other))
        );

        let retained_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(retained_tags, vec![tag(1), tag(2), tag(3)]);
        assert!(matches!(
            ingress.commands.back().map(|queued| &queued.command),
            Some(AdapterCommand::BodyAvailable { manifest }) if manifest.subject == subject
        ));
    }

    #[test]
    fn exact_authenticated_progress_retransmission_is_queue_coalesced() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"coalesced-progress-context",
            ))),
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-progress-block")),
            payload_hash: Hash::new(b"coalesced-progress-payload"),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"coalesced parent state"),
            Hash::new(b"coalesced post state"),
            Hash::new(b"coalesced ordinary writes"),
            Hash::new(b"coalesced executed block wire"),
        );
        let payload = wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        });
        let authenticated = || {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload.clone()))
        };
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(0), CommandClass::Progress, authenticated())
                .expect("first authenticated CommitQC owns one queue slot"),
            tag(0)
        );
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Progress, authenticated())
                .expect("equal authenticated retransmission is coalesced"),
            tag(0),
            "a coalesced retransmission returns the original queue owner's tag"
        );
        assert_eq!(ingress.len(), 1);

        let dispatched = ingress
            .pop_next()
            .expect("the sole queued CommitQC is dispatchable");
        assert_eq!(dispatched.class, CommandClass::Progress);
        assert!(matches!(
            dispatched.command,
            AdapterCommand::Authenticated(_)
        ));
        assert_eq!(ingress.len(), 0);

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated())
                .expect("a later retransmission starts a new ownership interval"),
            tag(2)
        );
        assert_eq!(ingress.len(), 1);
    }

    #[test]
    fn exact_authenticated_retransmission_preserves_capacity_fifo_and_cursor() {
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"coalesced-capacity-context",
            ))),
            height: 9,
            view: 4,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-capacity-block")),
            payload_hash: Hash::new(b"coalesced-capacity-payload"),
        };
        let payload = |signature| {
            wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"capacity parent state"),
                    Hash::new(b"capacity post state"),
                    Hash::new(b"capacity ordinary writes"),
                    Hash::new(b"capacity executed block wire"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![signature],
            })
        };
        let authenticated = |signature| {
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload(
                signature,
            )))
        };
        let queued_wire = wire::ConsensusMessageV2::new(payload(1));
        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new(b"coalesced capacity chunk")],
            chunk_root: Hash::new(b"coalesced capacity root"),
        });
        assert!(matches!(
            classify_reducer_network_ingress(false, &queued_wire.payload),
            Ok(CommandClass::Progress)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(false, &transport),
            Err(NetworkIngressError::TransportPayload)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &queued_wire.payload),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            classify_reducer_network_ingress(true, &transport),
            Err(NetworkIngressError::FailClosed)
        ));
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));

        assert_eq!(
            ingress
                .enqueue_authenticated(tag(0), CommandClass::Normal, authenticated(1))
                .expect("first wire value enters below the normal boundary"),
            tag(0)
        );
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(1), CommandClass::Normal, authenticated(2))
                .expect("a non-identical wire value uses ordinary capacity"),
            tag(1)
        );
        assert_eq!(
            ingress.check_capacity(CommandClass::Normal),
            Err(EnqueueError::ReservedCapacity)
        );

        let cursor_before = ingress.next_class;
        let tags_before = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(8), CommandClass::Normal, authenticated(1))
                .expect("an exact duplicate coalesces at reserved capacity"),
            tag(0),
            "coalescing deterministically returns the original admission tag"
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before,
            "coalescing changes neither FIFO ownership nor its tags"
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(9), CommandClass::Normal, authenticated(3)),
            Err(EnqueueError::ReservedCapacity),
            "a non-identical envelope still obeys the normal boundary"
        );

        ingress
            .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated(3))
            .expect("progress reserve remains independent");
        ingress
            .enqueue_authenticated(tag(3), CommandClass::Completion, authenticated(4))
            .expect("completion reserve fills the final slot");
        assert_eq!(ingress.len(), 4);
        assert_eq!(
            ingress.check_capacity(CommandClass::Completion),
            Err(EnqueueError::Full)
        );
        assert_eq!(ingress.authenticated_wire_tag(&queued_wire), Some(tag(0)));
        assert!(
            ingress
                .check_authenticated_wire_capacity(&queued_wire, CommandClass::Normal, false,)
                .is_ok(),
            "raw equality only opens the authentication attempt at full capacity"
        );
        assert_eq!(
            ingress.check_authenticated_wire_capacity(
                &wire::ConsensusMessageV2::new(payload(5)),
                CommandClass::Normal,
                false,
            ),
            Err(EnqueueError::Full)
        );

        let full_tags = ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        assert_eq!(
            ingress
                .enqueue_authenticated(tag(10), CommandClass::Normal, authenticated(1))
                .expect("the exact envelope coalesces even when every slot is owned"),
            tag(0)
        );
        assert_eq!(ingress.next_class, cursor_before);
        assert_eq!(
            ingress
                .commands
                .iter()
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            full_tags
        );
        assert!(
            ingress
                .commands
                .iter()
                .all(|queued| queued.eligible_skips == 0)
        );
        assert_eq!(
            ingress.enqueue_authenticated(tag(11), CommandClass::Progress, authenticated(5)),
            Err(EnqueueError::Full),
            "wire inequality cannot inherit the duplicate's full-queue exception"
        );
    }

    #[test]
    fn exact_authenticated_network_retransmission_obeys_runtime_boundaries() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let original = signed_runtime_proposal(&context, &keys, 1);
        let second = signed_runtime_proposal(&context, &keys, 2);
        let third = signed_runtime_proposal(&context, &keys, 3);
        let transport = match &original.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal) => wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadManifest(proposal.manifest.clone()),
            ),
            _ => unreachable!("fixture is a proposal"),
        };

        let owner_tag = runtime
            .enqueue_network(original.clone())
            .expect("first authenticated proposal owns one normal slot");
        assert_eq!(runtime.queued_commands(), 1);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces below the normal boundary"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 1);

        let mut invalid = third.clone();
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut invalid.payload else {
            unreachable!("fixture is a proposal")
        };
        proposal.signature[0] ^= 0x80;
        assert!(matches!(
            runtime.enqueue_network(invalid),
            Err(NetworkIngressError::Authentication(_))
        ));
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .enqueue_network(second.clone())
            .expect("non-identical authenticated proposal uses ordinary capacity");
        assert_eq!(runtime.queued_commands(), 2);
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact duplicate coalesces at reserved capacity"),
            owner_tag
        );
        assert!(matches!(
            runtime.enqueue_network(third.clone()),
            Err(NetworkIngressError::Backpressure(
                EnqueueError::ReservedCapacity
            ))
        ));

        let cursor_before = runtime.ingress.next_class;
        let tags_before = runtime
            .ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>();
        runtime
            .enqueue_signature(owner_tag, vec![4])
            .expect("completion reserve admits the third slot");
        runtime
            .enqueue_signature(owner_tag, vec![5])
            .expect("completion traffic may fill the fourth slot");
        assert_eq!(runtime.queued_commands(), 4);
        assert!(runtime.can_admit_network_message(&original));
        assert!(!runtime.can_admit_network_message(&third));
        assert_eq!(
            runtime
                .enqueue_network(original.clone())
                .expect("exact authenticated duplicate coalesces at full capacity"),
            owner_tag
        );
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(runtime.ingress.next_class, cursor_before);
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .take(tags_before.len())
                .map(|queued| queued.tag)
                .collect::<Vec<_>>(),
            tags_before
        );
        assert!(matches!(
            runtime.enqueue_network(third),
            Err(NetworkIngressError::Backpressure(EnqueueError::Full))
        ));

        runtime.fail_closed = true;
        assert!(matches!(
            runtime.enqueue_network(original.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        assert!(matches!(
            runtime.enqueue_network(transport.clone()),
            Err(NetworkIngressError::FailClosed)
        ));
        runtime.fail_closed = false;
        assert!(matches!(
            runtime.enqueue_network(transport),
            Err(NetworkIngressError::TransportPayload)
        ));
    }

    #[test]
    fn commit_certificate_response_waits_for_embedded_qc_progress_capacity() {
        let directory = TempDir::new().expect("temporary runtime ingress directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"response-capacity-block")),
            payload_hash: Hash::new(b"response-capacity-payload"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"response capacity parent state"),
                Hash::new(b"response capacity post state"),
                Hash::new(b"response capacity ordinary writes"),
                Hash::new(b"response capacity executed block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let response = |certificate| {
            wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                    wire::CommitCertificateResponse {
                        request_hash: HashOf::from_untyped_unchecked(Hash::new(
                            b"response capacity request",
                        )),
                        certificate,
                        responder: PeerId::new(keys[0].public_key().clone()),
                        signature: vec![1],
                    },
                ),
            )
        };
        let exact_response = response(certificate.clone());
        let mut distinct_certificate = certificate.clone();
        distinct_certificate.aggregate_signature = vec![2];
        let distinct_response = response(distinct_certificate);
        let owner_tag = runtime.round_tag();

        runtime
            .enqueue_signature(owner_tag, vec![3])
            .expect("first completion occupies shared capacity");
        runtime
            .enqueue_signature(owner_tag, vec![4])
            .expect("second completion occupies shared capacity");
        runtime
            .ingress
            .enqueue_authenticated(
                owner_tag,
                CommandClass::Progress,
                AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
                )),
            )
            .expect("authenticated CommitQC fills the Progress prefix");
        assert_eq!(runtime.queued_commands(), 3);

        assert!(
            !runtime.can_admit_network_message(&distinct_response),
            "a distinct response remains in outer ingress while inner Progress is full"
        );
        assert!(
            runtime.can_admit_network_message(&exact_response),
            "an exact embedded CommitQC can coalesce with its queued owner"
        );

        let released = runtime
            .ingress
            .pop_next()
            .expect("release one shared-capacity owner");
        assert_eq!(released.class, CommandClass::Completion);
        assert!(
            runtime.can_admit_network_message(&distinct_response),
            "the retained response can drain after Progress capacity returns"
        );
    }

    #[test]
    fn progress_is_not_starved_by_a_normal_traffic_flood() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 0..3 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }
        for value in 100..140 {
            assert_eq!(
                enqueue_fake(
                    &mut runtime,
                    initial,
                    CommandClass::Normal,
                    FakeCommand::record(value)
                ),
                Err(EnqueueError::ReservedCapacity)
            );
        }
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(200),
        )
        .expect("CommitQC/progress reserve remains available");

        let _ = runtime.step(start);
        assert_eq!(runtime.driver.delivered, vec![(initial, 200)]);
        let queue = runtime.queue_snapshot(start);
        assert_eq!(queue.normal.depth, 3);
        assert_eq!(queue.normal.capacity, 3);
        assert_eq!(queue.normal.max_service_debt, 1);
        assert_eq!(queue.progress.depth, 0);
        assert_eq!(queue.completion.depth, 0);
    }

    #[test]
    fn periodic_retransmit_cannot_starve_admitted_work_when_every_step_arrives_late() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        for value in 1..=2 {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .unwrap();
        }

        for seconds in [2, 4, 6, 8] {
            let _ = runtime.step(start + Duration::from_secs(seconds));
        }

        assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
        assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
    }

    #[test]
    fn absolute_timeout_preempts_admitted_work_owed_by_periodic_timer() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(7),
        )
        .unwrap();

        let _ = runtime.step(start + Duration::from_secs(2));
        let _ = runtime.step(start + Duration::from_secs(10));
        assert_eq!(runtime.driver.timeouts, vec![initial]);
        assert!(runtime.driver.delivered.is_empty());

        let _ = runtime.step(start + Duration::from_secs(12));
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
    }

    #[test]
    fn network_admission_uses_exact_normal_and_progress_reservations() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        let round = wire::ConsensusRound {
            context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"runtime-test-context",
            ))),
            height: 7,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime-test-block")),
            payload_hash: Hash::new(b"runtime-test-payload"),
        };
        let execution_commitment = wire::ExecutionCommitment::without_topups(
            Hash::new(b"runtime parent state"),
            Hash::new(b"runtime post state"),
            Hash::new(b"runtime ordinary writes"),
            Hash::new(b"runtime executed block wire"),
        );
        let vote = wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: vec![1],
        });
        let locked_commit_vote = match &vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.phase = wire::GlobalPhase::Commit;
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        runtime.driver.protected_commit = Some((round, subject, execution_commitment));
        let mismatched_commit_vote = match &locked_commit_vote {
            wire::ConsensusMessageV2Payload::Vote(vote) => {
                let mut vote = vote.clone();
                vote.subject.payload_hash = Hash::new(b"mismatched runtime commit vote");
                wire::ConsensusMessageV2Payload::Vote(vote)
            }
            _ => unreachable!("fixture is a vote"),
        };
        let certificate = wire::QuorumCertificate {
            round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let commit_qc = wire::ConsensusMessageV2Payload::QuorumCertificate(certificate.clone());
        let commit_response = wire::ConsensusMessageV2Payload::CommitCertificateResponse(
            wire::CommitCertificateResponse {
                request_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime commit request")),
                certificate,
                responder: PeerId::new(KeyPair::random().public_key().clone()),
                signature: vec![1],
            },
        );
        assert_eq!(network_command_class(&vote), Some(CommandClass::Normal));
        assert_eq!(
            network_command_class(&commit_qc),
            Some(CommandClass::Progress)
        );
        assert_eq!(network_command_class(&commit_response), None);
        assert_eq!(
            network_admission_class(&commit_response),
            Some(CommandClass::Progress)
        );
        assert!(runtime.can_admit_network_payload(&vote));
        assert!(runtime.can_admit_network_payload(&commit_qc));
        assert!(runtime.can_admit_network_payload(&commit_response));

        for value in [1, 2] {
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(value),
            )
            .expect("fill the normal prefix");
        }
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(
            !runtime.can_admit_network_payload(&mismatched_commit_vote),
            "a merely Commit-shaped vote must stop at pre-authentication backpressure"
        );
        assert!(
            runtime.can_admit_network_payload(&locked_commit_vote),
            "the exact locked Commit vote can reach authentication through the progress reserve"
        );
        assert!(
            runtime.can_admit_network_payload(&commit_qc),
            "CommitQC can use the reserved progress slot"
        );
        assert!(runtime.can_admit_network_payload(&commit_response));

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("fill the progress prefix");
        assert!(!runtime.can_admit_network_payload(&vote));
        assert!(!runtime.can_admit_network_payload(&mismatched_commit_vote));
        assert!(!runtime.can_admit_network_payload(&locked_commit_vote));
        assert!(!runtime.can_admit_network_payload(&commit_qc));
        assert!(!runtime.can_admit_network_payload(&commit_response));

        let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
            round,
            subject,
            payload_size_bytes: 1,
            layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1,
                max_chunk_count: 1,
            },
            chunk_hashes: vec![Hash::new([0_u8])],
            chunk_root: Hash::new(b"runtime transport root"),
        });
        assert!(runtime.can_admit_network_payload(&transport));
    }

    #[test]
    fn stale_completion_tag_is_delivered_after_due_retransmit_without_retagging() {
        let start = Instant::now();
        let current = tag(4);
        let stale = tag(2);
        let mut runtime = runtime(
            FakeDriver::new(current),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            stale,
            CommandClass::Completion,
            FakeCommand::record(9),
        )
        .unwrap();
        let _ = runtime.step(start + Duration::from_secs(2));
        assert_eq!(runtime.driver.retransmits, vec![current]);
        assert!(runtime.driver.delivered.is_empty());

        // Even though the clock remains retransmit-due, the admitted
        // completion is owed this slot and retains its original tag.
        let _ = runtime.step(start + Duration::from_secs(4));
        assert_eq!(runtime.driver.delivered, vec![(stale, 9)]);
    }

    #[test]
    fn only_enter_view_effect_restarts_both_clocks() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        let _ = runtime.step(start + Duration::from_secs(1));
        assert_eq!(runtime.round_tag(), initial);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::enter_view(next),
        )
        .unwrap();
        // Service the retransmission tick due at t=9, then the queued TC-like
        // progress command at the same monotonic instant.
        let _ = runtime.step(start + Duration::from_secs(9));
        let _ = runtime.step(start + Duration::from_secs(9));
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(22));

        assert!(matches!(
            runtime.step(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Idle)
        ));
        let _ = runtime.step(start + Duration::from_secs(11));
        assert_eq!(runtime.driver.retransmits, vec![initial, next]);
        let _ = runtime.step(start + Duration::from_secs(19));
        assert!(runtime.driver.timeouts.is_empty());
        let _ = runtime.step(start + Duration::from_secs(29));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn startup_enter_view_effect_restarts_clocks_and_is_returned_unchanged() {
        let start = Instant::now();
        let initial = tag(0);
        let next = tag(1);
        let (mut runtime, effects) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![FakeEffect::enter_view(next), FakeEffect::other()],
        )
        .unwrap();
        assert_eq!(runtime.round_tag(), next);
        assert_eq!(runtime.round_timeout(), Duration::from_secs(20));
        assert_eq!(
            effects,
            vec![FakeEffect::enter_view(next), FakeEffect::other()]
        );
        assert!(matches!(
            runtime.step(start + Duration::from_secs(100)),
            Err(RuntimeError::ClocksNotArmed)
        ));
        runtime
            .arm_live_clocks(start + Duration::from_secs(100))
            .expect("arm after startup effects are dispatched");
        assert_eq!(
            runtime.arm_live_clocks(start + Duration::from_secs(101)),
            Err(RuntimeClockError::AlreadyArmed)
        );
        assert!(matches!(
            runtime.step(start + Duration::from_secs(119)),
            Ok(RuntimeStep::Advanced(_)) | Ok(RuntimeStep::Idle)
        ));
        assert!(runtime.driver.timeouts.is_empty());
        let _ = runtime.step(start + Duration::from_secs(120));
        assert_eq!(runtime.driver.timeouts, vec![next]);
    }

    #[test]
    fn interrupted_tip_recovery_drains_ingress_without_arming_live_timers() {
        let start = Instant::now();
        let initial = tag(0);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("queue local recovery completion");

        assert!(matches!(
            runtime.step_recovery(start + Duration::from_secs(1_000)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(runtime.driver.delivered, vec![(initial, 7)]);
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());
        assert!(matches!(
            runtime.step_recovery(start + Duration::from_secs(2_000)),
            Ok(RuntimeStep::Idle)
        ));
    }

    #[test]
    fn interrupted_tip_recovery_is_rejected_after_live_clock_arm() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );

        assert!(matches!(
            runtime.step_recovery(start),
            Err(RuntimeError::RecoveryAfterClocksArmed)
        ));
    }

    #[test]
    fn adapter_failure_closes_runtime_permanently() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(5, 1, 1),
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::fail(),
        )
        .unwrap();
        assert!(matches!(
            runtime.step(start),
            Err(RuntimeError::Driver(FakeError))
        ));
        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
    }

    #[test]
    fn invalid_configuration_is_rejected() {
        let start = Instant::now();
        let initial = tag(0);
        let result = SerializedV2Runtime::with_driver(
            FakeDriver::new(initial),
            start,
            Duration::ZERO,
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::<FakeEffect>::new(),
        );
        assert!(matches!(
            result,
            Err(RuntimeConfigError::InvalidRoundTimeout)
        ));

        let invalid_queue = RuntimeQueueConfig::new(2, 1, 1).validate();
        assert_eq!(
            invalid_queue,
            Err(RuntimeConfigError::InvalidQueueAllocation)
        );
    }
}
