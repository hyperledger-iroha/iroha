//! Live obligations that restrictions must not strand (`specs/live_runtime_upgrades.md` §7.2).
//!
//! Each live obligation's committed record is its registration:
//!
//! - **Escrows.** An asset escrow or lock whose status is still open (`world.asset_escrows`): its
//!   funds sit in protocol custody until one of its terminal instructions runs.
//! - **AMX participations** (`specs/sumeragi.md` §11). On the global chain, a transaction whose
//!   `Begin` is recorded and that is not decided yet (`world.sumeragi_amx`); on a participant
//!   root, an escrow of a `Yes` vote that no decision has settled yet
//!   (`world.sumeragi_amx_participant`). A `No` vote escrows nothing, so its entry is not live.
//!
//! - **Selected SoraFS authorities.** Granted heads, pending proposals and unprocessed leases
//!   retain the exact selected authority services and original completion closure.
//!
//! A restriction never removes settlement authority (§7.1):
//!
//! - while a definition behind an instruction has admission closed, the instructions that
//!   continue or complete a live obligation still run, because work already admitted continues;
//! - while it is completion-only, the instructions that complete a live obligation still run;
//! - an instruction that starts new work runs under neither;
//! - unselected native escrow, AMX and SoraFS authority records still require their original
//!   bindings and dependency closure. Rebinding those classes requires draining them first;
//!   selected native escrow captures and registered obligations execute retained bindings.
//!
//! **Content cleanup.** On a root that selected the content-publishing permission, revoking a
//! direct grant an account holds and retiring a bundle published after genesis complete work
//! already admitted ([`content_step`]), so they run under closed admission and completion-only.
//! Grants and bundles are durable State without custody, so they are not live obligations: they
//! pin no closure and never hold back a rebinding, a kernel or profile change, or an executor
//! replacement. `content_publish_permission::check_cleanup` only keeps both instructions
//! executable while such State remains. An unselected root keeps the frozen baseline's rules.
//!
//! **Escrow steps.** Accepting an offer continues an escrow and adds the buyer's exposure. Marking
//! payment sent, releasing, cancelling, opening or resolving a dispute, attesting a condition,
//! drawing down and expiring complete it. Opening one is new work.
//! The selected native escrow service ([`super::native_escrow_capture`]) captures canonical
//! custody semantics before funding and retains them until the authoritative terminal write.
//! Direct operations run their captured graph with their own step. Batched orderbook custody
//! continues to pin its predecessor graph, and checks actual scoped execution before movement.
//!
//! **AMX steps.**
//!
//! | Instruction | Acts on | Step |
//! | --- | --- | --- |
//! | `RegisterAmxDataspaceV1`, `RegisterAmxParticipantV1` | a registration | new work |
//! | `BeginAmxV1` | a new transaction | new work |
//! | `PrepareAmxV1` | new custody of the participant | new work |
//! | `RelayAmxPreparedV1`, `ExpireAmxV1` | its undecided transaction | completes |
//! | `RelayAmxHandoffV1` | the undecided transactions its dataspace takes part in | completes |
//! | `SettleAmxV1` | the participant's unsettled escrow of the decided transaction | completes |
//! | `RelayGlobalAmxHandoffV1` | the participant's unsettled escrows | completes |
//!
//! A participant cannot tie a `Prepare` to an admitted transaction before verifying its `Begin`
//! proof, and the escrow it creates is new custody on its root. Refusing it strands nothing: the
//! global chain aborts the transaction by its deadline, and the participants that voted `Yes`
//! release through `Settle`. Nothing the coordinator does adds exposure: a relayed vote records
//! custody a participant already escrowed, a vote or an expiry decides the transaction, and a
//! handoff keeps the votes of its participants verifiable. `Settle` is the only way a `Yes` escrow
//! is released, and a decision certified in a later global epoch verifies only after the
//! participant relayed the handoffs up to it. The deadline step is a block hook of the services
//! profile, not an instruction, and aborts whatever is still undecided after the deadline. AMX
//! completions are native and invoke no IVM contract, so unlike controlled locks they retain no
//! host profile.
//!
//! **Registered obligations.** An account, or the account that controls a contract's lifecycle,
//! registers an obligation with the kernel ([`registry`]): the record in
//! `world.runtime_obligations` commits to the exact instructions that complete it and captures
//! their completion closure from the selection the root executes at that moment. When the owner
//! issues one of those instructions, it and what it invokes complete the obligation, once. A
//! registered obligation is live until it expires or has performed every committed instruction,
//! and executes in a scoped context using its captured bindings. A change set may rebind those
//! interfaces while retaining the captured code and storage. Removing required definitions or
//! replacing its root-wide kernel, commitment or services profile still lapses.
//!
//! **Migration cursors.** A migration is the kernel's own obligation ([`super::migration`]): its
//! record in `world.runtime_migrations` holds the cursor until the cutover.
//!
//! Pending multisig consent and scheduled triggers follow the selected deferred-consent service
//! ([`super::deferred_consent`]); they suspend on changed or restricted captures instead of
//! receiving a settlement capability. Selected native AMX captures retain the original decision,
//! settlement and shared-tracker closure; automatic expiry belongs to their pinned service.

use std::collections::BTreeSet;

use iroha_data_model::{
    escrow::{AssetEscrowStatus, EscrowId},
    isi::{
        InstructionBox,
        escrow::{
            AcceptAssetEscrow, AttestEscrowCondition, CancelAssetEscrow, CancelAssetLock,
            DrawdownAssetLock, ExpireAssetLock, ExpireConditionalEscrow, MarkEscrowPaymentSent,
            OpenEscrowDispute, ReleaseAssetEscrow, ResolveEscrowDispute,
        },
        escrow_control::{
            CancelControlledAssetLockV1, DrawdownControlledAssetLockV1, ExpireControlledAssetLockV1,
        },
        sumeragi_amx::{
            ExpireAmxV1, RelayAmxHandoffV1, RelayAmxPreparedV1, RelayGlobalAmxHandoffV1,
            SettleAmxV1,
        },
        wire_id_of,
    },
    runtime::{
        definition::{
            BindingV1, DefinitionDescriptorV1, DefinitionId, DefinitionLookup, RestrictionV1,
            RootSelectionV1,
        },
        lifecycle::ObligationKindV1,
    },
    sumeragi_amx::{
        AmxPreparedEntryV1, AmxRecordV1, AmxVoteV1, NativeAmxParticipantStateV1, SumeragiAmxState,
    },
};
use iroha_model_base::topology::DataSpaceId;
use mv::storage::StorageReadOnly as _;

pub(crate) mod registry;

/// How an instruction acts on a live obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// It advances the obligation and adds a party's exposure, such as a buyer accepting an
    /// offer: work already admitted continues, but completion-only does not run it.
    Continue,
    /// It completes the obligation, or protects a party on the way to completing it, and adds no
    /// new exposure.
    Complete,
}

impl Step {
    /// Whether this step runs on a definition under `restriction` (§7.1). Deprecation never stops
    /// execution, and disabling stops it entirely.
    pub(crate) const fn runs_under(self, restriction: RestrictionV1) -> bool {
        match restriction {
            RestrictionV1::Deprecated(_) | RestrictionV1::AdmissionClosed(_) => true,
            RestrictionV1::CompletionOnly(_) => matches!(self, Self::Complete),
            RestrictionV1::Disabled(_) => false,
        }
    }
}

/// Whether an escrow in `status` is still a live obligation: its custody has not been paid out,
/// refunded or resolved.
pub const fn is_live(status: AssetEscrowStatus) -> bool {
    match status {
        AssetEscrowStatus::Open
        | AssetEscrowStatus::Accepted
        | AssetEscrowStatus::PaymentSent
        | AssetEscrowStatus::Disputed
        | AssetEscrowStatus::Locked => true,
        AssetEscrowStatus::Released
        | AssetEscrowStatus::Cancelled
        | AssetEscrowStatus::Resolved
        | AssetEscrowStatus::DrawnDown
        | AssetEscrowStatus::Expired => false,
    }
}

/// The escrow `instruction` acts on and how. Instructions that open new custody, and every
/// instruction that is not an escrow instruction, act on no live escrow.
pub fn escrow_step(instruction: &InstructionBox) -> Option<(EscrowId, Step)> {
    let instruction = instruction.as_any();
    let step = |escrow_id: &EscrowId, step| Some((*escrow_id, step));
    if let Some(accept) = instruction.downcast_ref::<AcceptAssetEscrow>() {
        return step(&accept.escrow_id, Step::Continue);
    }
    if let Some(mark) = instruction.downcast_ref::<MarkEscrowPaymentSent>() {
        return step(&mark.escrow_id, Step::Complete);
    }
    if let Some(release) = instruction.downcast_ref::<ReleaseAssetEscrow>() {
        return step(&release.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelAssetEscrow>() {
        return step(&cancel.escrow_id, Step::Complete);
    }
    if let Some(dispute) = instruction.downcast_ref::<OpenEscrowDispute>() {
        return step(&dispute.escrow_id, Step::Complete);
    }
    if let Some(resolve) = instruction.downcast_ref::<ResolveEscrowDispute>() {
        return step(&resolve.escrow_id, Step::Complete);
    }
    if let Some(attest) = instruction.downcast_ref::<AttestEscrowCondition>() {
        return step(&attest.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireConditionalEscrow>() {
        return step(&expire.escrow_id, Step::Complete);
    }
    if let Some(drawdown) = instruction.downcast_ref::<DrawdownAssetLock>() {
        return step(&drawdown.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelAssetLock>() {
        return step(&cancel.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireAssetLock>() {
        return step(&expire.escrow_id, Step::Complete);
    }
    if let Some(drawdown) = instruction.downcast_ref::<DrawdownControlledAssetLockV1>() {
        return step(&drawdown.drawdown.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelControlledAssetLockV1>() {
        return step(&cancel.cancel.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireControlledAssetLockV1>() {
        return step(&expire.expiry.escrow_id, Step::Complete);
    }
    None
}

/// What an AMX instruction acts on (`specs/sumeragi.md` §11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmxParticipation {
    /// A transaction of the global chain, live until it is decided.
    Decision([u8; 32]),
    /// The transactions of the global chain a dataspace takes part in, live while one of them is
    /// undecided: relaying the dataspace's handoff keeps their votes verifiable.
    DecisionsOf(DataSpaceId),
    /// A participant's escrow of a transaction, live until a decision settles it.
    Settlement {
        /// The participant.
        dataspace: DataSpaceId,
        /// The transaction.
        tx: [u8; 32],
    },
    /// A participant's escrows, live while one of them is unsettled: relaying the global chain's
    /// handoff keeps their decisions verifiable.
    SettlementsOf(DataSpaceId),
}

impl AmxParticipation {
    /// Whether this participation is live in a root's coordinator state and in its native
    /// participant state, if the root has one.
    pub(crate) fn is_live(
        self,
        coordinator: &SumeragiAmxState,
        participant: Option<&NativeAmxParticipantStateV1>,
    ) -> bool {
        let participant_of = |dataspace: DataSpaceId| {
            participant.filter(|state| state.participant.dataspace == dataspace)
        };
        match self {
            Self::Decision(tx) => coordinator
                .transaction(&tx)
                .is_some_and(|entry| entry.decided.is_none()),
            Self::DecisionsOf(dataspace) => coordinator
                .transactions
                .iter()
                .any(|entry| entry.decided.is_none() && entry.begin.has_participant(dataspace)),
            Self::Settlement { dataspace, tx } => {
                participant_of(dataspace).is_some_and(|state| unsettled(state, &tx))
            }
            Self::SettlementsOf(dataspace) => participant_of(dataspace).is_some_and(|state| {
                state.escrows.iter().any(|record| record.settled.is_none())
                    || state.participant.prepared.iter().any(unsettled_yes)
            }),
        }
    }
}

/// Whether a prepared entry voted `Yes` and no decision has settled it yet.
fn unsettled_yes(entry: &AmxPreparedEntryV1) -> bool {
    entry.settled.is_none() && matches!(entry.vote, AmxVoteV1::Yes(_))
}

/// Whether the participant's custody record of `tx` exists and no decision has settled it yet.
fn unsettled_custody(state: &NativeAmxParticipantStateV1, tx: &[u8; 32]) -> bool {
    state
        .escrows
        .binary_search_by(|record| record.tx.cmp(tx))
        .is_ok_and(|index| state.escrows[index].settled.is_none())
}

/// Whether the participant's escrow of `tx` awaits its decision: its custody record or its `Yes`
/// vote is unsettled.
fn unsettled(state: &NativeAmxParticipantStateV1, tx: &[u8; 32]) -> bool {
    unsettled_custody(state, tx) || state.participant.entry(tx).is_some_and(unsettled_yes)
}

/// How many of the participant's escrows await their decision. A valid participant keeps one
/// custody record per `Yes` vote; a vote without an unsettled record still counts, once.
fn unsettled_escrows(state: &NativeAmxParticipantStateV1) -> usize {
    let custody = state
        .escrows
        .iter()
        .filter(|record| record.settled.is_none())
        .count();
    let votes = state
        .participant
        .prepared
        .iter()
        .filter(|entry| unsettled_yes(entry) && !unsettled_custody(state, &entry.tx))
        .count();
    custody + votes
}

/// The AMX participation `instruction` acts on and how. Registering, beginning and preparing
/// start new work, and every instruction that is not an AMX instruction acts on no participation;
/// every other AMX instruction completes one. A proof of the wrong record kind acts on nothing:
/// its execution rejects it.
pub fn amx_step(instruction: &InstructionBox) -> Option<(AmxParticipation, Step)> {
    let instruction = instruction.as_any();
    let participation = if let Some(relay) = instruction.downcast_ref::<RelayAmxPreparedV1>() {
        let AmxRecordV1::Prepared(prepared) = &relay.proof.record else {
            return None;
        };
        AmxParticipation::Decision(prepared.tx)
    } else if let Some(expire) = instruction.downcast_ref::<ExpireAmxV1>() {
        AmxParticipation::Decision(expire.tx)
    } else if let Some(handoff) = instruction.downcast_ref::<RelayAmxHandoffV1>() {
        AmxParticipation::DecisionsOf(handoff.dataspace)
    } else if let Some(settle) = instruction.downcast_ref::<SettleAmxV1>() {
        let AmxRecordV1::Decision(decision) = &settle.decision.record else {
            return None;
        };
        AmxParticipation::Settlement {
            dataspace: settle.dataspace,
            tx: decision.tx,
        }
    } else {
        let handoff = instruction.downcast_ref::<RelayGlobalAmxHandoffV1>()?;
        AmxParticipation::SettlementsOf(handoff.dataspace)
    };
    Some((participation, Step::Complete))
}

/// How `instruction` acts on a live obligation of `world`, if it does.
pub fn step(
    instruction: &InstructionBox,
    world: &crate::state::WorldTransaction<'_, '_>,
) -> Option<Step> {
    if let Some((escrow_id, step)) = escrow_step(instruction) {
        return world
            .asset_escrows
            .get(&escrow_id)
            .is_some_and(|record| is_live(record.status))
            .then_some(step);
    }
    if let Some(step) = content_step(instruction, world) {
        return Some(step);
    }
    if let Some(step) = authority_step(instruction, world) {
        return Some(step);
    }
    if let Some(exercise) = instruction
        .as_any()
        .downcast_ref::<iroha_data_model::isi::runtime_obligation::CompleteRuntimeObligationV1>(
    ) {
        // Ending a registered obligation completes it; its handler decides who may. A root
        // that holds no obligation skips the identity derivation.
        return (world.runtime_obligations.iter().next().is_some()
            && iroha_data_model::runtime::obligation::ObligationId::derive(&exercise.owner)
                .is_ok_and(|id| world.runtime_obligations.get(&id).is_some()))
        .then_some(Step::Complete);
    }
    let (participation, step) = amx_step(instruction)?;
    participation
        .is_live(
            world.sumeragi_amx.get(),
            world.sumeragi_amx_participant.get().canonical(),
        )
        .then_some(step)
}

/// Cleanup of governed content State as completion work: retiring an existing bundle published
/// after genesis, or revoking a direct grant an account holds in exactly that form, on a root
/// that selected the content-publishing permission. Authority and payload checks still belong to
/// the executor and the native handlers.
fn content_step(
    instruction: &InstructionBox,
    world: &crate::state::WorldTransaction<'_, '_>,
) -> Option<Step> {
    use iroha_data_model::{
        account::Account,
        isi::{Revoke, RevokeBox, content::RetireContentBundle},
        permission::Permission,
    };

    if !super::content_publish_permission::selected(world.runtime_selection.get()) {
        return None;
    }
    let raw = instruction.as_any();
    if let Some(retire) = raw.downcast_ref::<RetireContentBundle>() {
        return world
            .content_bundles
            .get(&retire.bundle_id)
            .is_some_and(|record| {
                record.created_height > super::content_publish_permission::GENESIS_HEIGHT
            })
            .then_some(Step::Complete);
    }
    let revoke = raw
        .downcast_ref::<Revoke<Permission, Account>>()
        .or_else(|| match raw.downcast_ref::<RevokeBox>() {
            Some(RevokeBox::Permission(revoke)) => Some(revoke),
            _ => None,
        })?;
    (revoke.object.name() == super::content_publish_permission::NAME
        && world
            .account_permissions
            .get(&revoke.destination)
            .is_some_and(|permissions| permissions.contains(&revoke.object)))
    .then_some(Step::Complete)
}

/// How selected authority work continues or completes, without changing escrow or AMX admission.
fn authority_step(
    instruction: &InstructionBox,
    world: &crate::state::WorldTransaction<'_, '_>,
) -> Option<Step> {
    if super::sorafs_authority_enabled(world.runtime_selection.get()).ok() == Some(true) {
        use iroha_data_model::isi::sorafs_authority::{
            CreateSorafsAuthorityParliamentAttemptV1, ProposeSorafsAuthorityGovernanceV1,
            SorafsAuthorityActionV1,
        };
        let raw = instruction.as_any();
        let proposal = raw
            .downcast_ref::<ProposeSorafsAuthorityGovernanceV1>()
            .map(|instruction| &instruction.proposal)
            .or_else(|| {
                raw.downcast_ref::<CreateSorafsAuthorityParliamentAttemptV1>()
                    .map(|instruction| &instruction.proposal)
            });
        if let Some(proposal) = proposal {
            if proposal.action == SorafsAuthorityActionV1::Revoke
                && world
                    .sorafs_authority_heads
                    .get(&proposal.governed_subject_id_v1())
                    .is_some_and(|head| {
                        head.granted
                            && head.plan_id == proposal.plan_id()
                            && proposal.expected_revision.checked_add(1) == Some(head.revision)
                    })
            {
                return Some(Step::Complete);
            }
            if world
                .sorafs_authority_proposals
                .get(&proposal.fingerprint())
                .is_some_and(|record| {
                    record.status == crate::state::GovernanceProposalStatus::Proposed
                        && record.deadline_closure.is_none()
                })
            {
                // Finishing already admitted proposal content closes its Parliament
                // obligation, including a vote that rejects the requested grant.
                return Some(Step::Complete);
            }
        }
        if let Some(transition) = raw.downcast_ref::<iroha_data_model::isi::governance::SubmitParliamentLifecycleTransitionV1>()
            && let Some(attempt) = world.parliament_attempts.get(&transition.governance_attempt_id)
            && let Some(record) = world.sorafs_authority_proposals.get(attempt.proposal_content_id().as_bytes()) {
            if record.status == crate::state::GovernanceProposalStatus::Proposed && record.deadline_closure.is_none() {
                return Some(Step::Complete);
            }
        }
    }
    None
}

/// Wire IDs of the instructions that complete a live escrow. The successor of a change set must
/// keep each one bound to an active, enabled interface and implementation while escrows are live.
fn escrow_completions() -> Vec<&'static str> {
    [
        wire_id_of::<MarkEscrowPaymentSent>(),
        wire_id_of::<ReleaseAssetEscrow>(),
        wire_id_of::<CancelAssetEscrow>(),
        wire_id_of::<OpenEscrowDispute>(),
        wire_id_of::<ResolveEscrowDispute>(),
        wire_id_of::<AttestEscrowCondition>(),
        wire_id_of::<ExpireConditionalEscrow>(),
        wire_id_of::<DrawdownAssetLock>(),
        wire_id_of::<CancelAssetLock>(),
        wire_id_of::<ExpireAssetLock>(),
        wire_id_of::<DrawdownControlledAssetLockV1>(),
        wire_id_of::<CancelControlledAssetLockV1>(),
        wire_id_of::<ExpireControlledAssetLockV1>(),
    ]
    .into_iter()
    .map(|wire_id| wire_id.expect("escrow completions are registered built-in instructions"))
    .collect()
}

/// Wire IDs of the instructions through which the global chain decides a live AMX transaction.
const AMX_DECISION_COMPLETIONS: [&str; 3] = [
    RelayAmxPreparedV1::WIRE_ID,
    ExpireAmxV1::WIRE_ID,
    RelayAmxHandoffV1::WIRE_ID,
];

/// Wire IDs of the instructions through which a participant settles a live AMX escrow.
const AMX_SETTLEMENT_COMPLETIONS: [&str; 2] =
    [SettleAmxV1::WIRE_ID, RelayGlobalAmxHandoffV1::WIRE_ID];

/// Live obligations whose completion needs the same instructions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// Native custody that still pins predecessor bindings, including orderbook batches.
    Escrow,
    /// AMX transactions the global chain has not decided.
    AmxDecision,
    /// AMX escrows a participant has not settled.
    AmxSettlement,
}

impl Class {
    /// Every class, in the order [`stranded`] checks them.
    const ALL: [Self; 3] = [Self::Escrow, Self::AmxDecision, Self::AmxSettlement];

    /// The kind of the class's obligations and what they await.
    const fn kind(self) -> (ObligationKindV1, &'static str) {
        match self {
            Self::Escrow => (ObligationKindV1::Escrow, ""),
            Self::AmxDecision => (ObligationKindV1::AmxParticipation, " awaiting a decision"),
            Self::AmxSettlement => (ObligationKindV1::AmxParticipation, " awaiting settlement"),
        }
    }

    /// Wire IDs of the instructions that complete the class's obligations.
    fn completions(self) -> Vec<&'static str> {
        match self {
            Self::Escrow => escrow_completions(),
            Self::AmxDecision => AMX_DECISION_COMPLETIONS.to_vec(),
            Self::AmxSettlement => AMX_SETTLEMENT_COMPLETIONS.to_vec(),
        }
    }

    /// Whether the completions' native code can invoke IVM contracts, so the predecessor's
    /// admitted host profiles stay executable too: controlled locks can, AMX cannot.
    const fn invokes_contracts(self) -> bool {
        matches!(self, Self::Escrow)
    }
}

/// Live obligations of committed State, counted by what their completion needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LiveObligations {
    /// Asset escrows and locks in custody.
    escrows: usize,
    /// Selected orderbook custody whose native terminal parents require ordinary admission.
    orderbook_custody: usize,
    /// Exact original closures of selected native escrow custody.
    captured_escrows: Vec<(
        iroha_data_model::runtime::obligation::CompletionCaptureV1,
        usize,
    )>,
    /// Exact original decision and settlement closures of selected native AMX owners.
    captured_amx: Vec<(
        iroha_data_model::runtime::obligation::CompletionCaptureV1,
        usize,
    )>,
    /// AMX transactions this root coordinates and has not decided.
    amx_decisions: usize,
    /// AMX escrows this root holds as a participant and has not settled.
    amx_settlements: usize,
    /// Selected SoraFS authority custody and pending Parliament certificates.
    sorafs_authorities: usize,
    /// The distinct completion closures of the live obligations registered through the kernel,
    /// each with how many obligations share it.
    registered: Vec<(
        iroha_data_model::runtime::obligation::CompletionCaptureV1,
        usize,
    )>,
}

impl LiveObligations {
    /// These obligations with the obligations registered through the kernel that are live in
    /// `world` at committed time `now_ms`.
    #[must_use]
    pub(crate) fn with_registered(
        mut self,
        world: &crate::state::WorldTransaction<'_, '_>,
        now_ms: u64,
    ) -> Self {
        self.registered = registry::live_captures(world, now_ms);
        self
    }

    /// The custody and participation obligations of `world`: escrows, AMX participations and
    /// selected SoraFS authorities. [`Self::with_registered`] adds the registered ones.
    pub(crate) fn of(world: &crate::state::WorldTransaction<'_, '_>) -> Self {
        let sorafs_authorities =
            if super::sorafs_authority_enabled(world.runtime_selection.get()).ok() == Some(true) {
                world
                    .sorafs_authority_heads
                    .iter()
                    .filter(|(_, head)| head.granted)
                    .count()
                    + world
                        .sorafs_authority_proposals
                        .iter()
                        .filter(|(_, record)| {
                            matches!(
                                record.status,
                                crate::state::GovernanceProposalStatus::Proposed
                            ) && record.deadline_closure.is_none()
                        })
                        .count()
                    + world
                        .sorafs_authority_leases
                        .iter()
                        .filter(|(_, lease)| !lease.start_processed)
                        .count()
            } else {
                0
            };
        let mut amx = Self::amx(
            world.sumeragi_amx.get(),
            world.sumeragi_amx_participant.get().canonical(),
        );
        // An invalid or incomplete capture set never weakens the old conservative graph pin.
        if super::native_amx_capture::enabled(world.runtime_selection.canonical()).ok()
            == Some(true)
            && super::native_amx_capture::validate_persistence(&*world).is_ok()
        {
            amx.amx_decisions = 0;
            amx.amx_settlements = 0;
            amx.captured_amx = super::native_amx_capture::live_captures(world);
        }
        Self {
            sorafs_authorities,
            registered: Vec::new(),
            captured_escrows: super::native_escrow_capture::live_captures(world),
            orderbook_custody: super::native_escrow_capture::live_orderbook_count(world),
            escrows: world
                .asset_escrows
                .iter()
                .filter(|(id, record)| {
                    is_live(record.status)
                        && super::native_escrow_capture::pins_predecessor(world, id)
                })
                .count(),
            ..amx
        }
    }

    /// The live AMX participations of a root's coordinator state and of its native participant
    /// state, if it has one.
    fn amx(
        coordinator: &SumeragiAmxState,
        participant: Option<&NativeAmxParticipantStateV1>,
    ) -> Self {
        Self {
            escrows: 0,
            orderbook_custody: 0,
            captured_escrows: Vec::new(),
            captured_amx: Vec::new(),
            amx_decisions: coordinator
                .transactions
                .iter()
                .filter(|entry| entry.decided.is_none())
                .count(),
            amx_settlements: participant.map_or(0, unsettled_escrows),
            sorafs_authorities: 0,
            registered: Vec::new(),
        }
    }

    /// Whether no obligation is live.
    pub(crate) fn is_empty(&self) -> bool {
        self.sorafs_authorities == 0
            && self.orderbook_custody == 0
            && self.registered.is_empty()
            && self.captured_escrows.is_empty()
            && self.captured_amx.is_empty()
            && Class::ALL.into_iter().all(|class| self.count(class) == 0)
    }

    /// How many live obligations of `class` there are.
    const fn count(&self, class: Class) -> usize {
        match class {
            Class::Escrow => self.escrows,
            Class::AmxDecision => self.amx_decisions,
            Class::AmxSettlement => self.amx_settlements,
        }
    }
}

#[cfg(test)]
impl LiveObligations {
    /// `escrows` live escrow obligations and nothing else.
    pub(crate) fn with_escrows(escrows: usize) -> Self {
        Self {
            escrows,
            ..Self::default()
        }
    }

    /// `decisions` undecided and `settlements` unsettled AMX participations and nothing else.
    pub(crate) fn with_amx(decisions: usize, settlements: usize) -> Self {
        Self {
            escrows: 0,
            orderbook_custody: 0,
            captured_escrows: Vec::new(),
            captured_amx: Vec::new(),
            amx_decisions: decisions,
            amx_settlements: settlements,
            sorafs_authorities: 0,
            registered: Vec::new(),
        }
    }
}

/// Whether `selection` retains `definition` in its execution closure and does not disable it.
fn executes(selection: &RootSelectionV1, definition: DefinitionId) -> bool {
    let disabled = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(&definition))
        .is_ok_and(|index| {
            matches!(
                selection.restrictions[index].restriction,
                RestrictionV1::Disabled(_)
            )
        });
    let retained = definition == selection.kernel
        || definition == selection.commitment_profile
        || definition == selection.services_profile
        || selection.active.binary_search(&definition).is_ok()
        || selection
            .installed_storage
            .binary_search(&definition)
            .is_ok();
    retained && !disabled
}

/// The selection's binding of `wire_id`.
fn binding<'a>(selection: &'a RootSelectionV1, wire_id: &str) -> Option<&'a BindingV1> {
    selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .ok()
        .map(|index| &selection.bindings[index])
}

/// Whether `selection` binds `wire_id` to an interface and an implementation it executes: both
/// retained and neither disabled. Closed admission and completion-only still execute.
pub(super) fn executable(selection: &RootSelectionV1, wire_id: &str) -> bool {
    binding(selection, wire_id).is_some_and(|bound| {
        executes(selection, bound.interface) && executes(selection, bound.implementation)
    })
}

/// Why `successor` changes the predecessor's binding of `wire_id`, the `role` it plays in a live
/// obligation's completion, if it does. A binding stays identical, or unbound if it was unbound:
/// binding new code is not the original settlement authority either.
fn binding_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    role: &str,
    wire_id: &str,
) -> Option<String> {
    let original = binding(predecessor, wire_id);
    let next = binding(successor, wire_id);
    if original == next {
        return None;
    }
    Some(match (original, next) {
        (_, None) => format!("{role} {wire_id} is unbound"),
        (None, Some(_)) => {
            format!("{role} {wire_id} is bound without a captured completion capability")
        }
        (Some(_), Some(_)) => {
            format!("{role} {wire_id} is rebound without a captured completion capability")
        }
    })
}

/// Why `successor` cannot keep the original closure of `completions` executable, if it cannot.
///
/// The original closure is what the predecessor's bindings of `completions` reach: their
/// interfaces and implementations, the definitions those declare, and the bindings of the
/// interfaces they declare, transitively; with the kernel, the commitment and services profiles
/// and, when `host_profiles` holds, every host profile the predecessor admits. The successor
/// keeps every binding in it identical and keeps executing every definition in it that the
/// predecessor executes. The baseline native bundle conservatively declares all interfaces it can
/// invoke: keeping a direct completion binding while changing one of its callees changes its
/// semantics just as surely as rebinding the completion itself. Following descriptor identities
/// also retains its storage, codecs, verification rules and resource limits.
///
/// What the predecessor already leaves unbound or disabled is no part of a live obligation's
/// executable closure, so leaving it so blocks no change set.
fn closure_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl DefinitionLookup,
    completions: &[&str],
    host_profiles: bool,
) -> Option<String> {
    let mut pending = BTreeSet::from([
        predecessor.kernel,
        predecessor.commitment_profile,
        predecessor.services_profile,
    ]);
    if host_profiles {
        pending.extend(predecessor.active.iter().copied().filter(|id| {
            matches!(
                published.definition(id),
                Some(DefinitionDescriptorV1::HostProfile(_))
            )
        }));
    }
    for wire_id in completions {
        if let Some(gap) = binding_gap(predecessor, successor, "completion", wire_id) {
            return Some(gap);
        }
        if let Some(original) = binding(predecessor, wire_id) {
            pending.extend([original.interface, original.implementation]);
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(definition) = pending.pop_first() {
        if !visited.insert(definition) {
            continue;
        }
        if executes(predecessor, definition) && !executes(successor, definition) {
            return Some(format!("needs {definition}, which is inactive or disabled"));
        }
        let Some(descriptor) = published.definition(&definition) else {
            return Some(format!("needs unpublished definition {definition}"));
        };
        if let DefinitionDescriptorV1::Interface(interface) = descriptor {
            let wire_id = interface.wire_id.as_str();
            if let Some(gap) = binding_gap(predecessor, successor, "dependency", wire_id) {
                return Some(gap);
            }
            if let Some(original) = binding(predecessor, wire_id) {
                pending.insert(original.implementation);
            }
        }
        pending.extend(
            descriptor
                .typed_dependencies()
                .into_iter()
                .map(|(_, id)| id),
        );
    }
    None
}

/// Why `successor` would strand `live` obligations, if it would (§7.2).
///
/// Unselected native escrows, orderbook batches, AMX and SoraFS authorities retain their
/// predecessor bindings. Selected native escrow captures and registered obligations may execute
/// retained code beside a replacement, but cannot lose required definitions, storage or root-wide
/// profiles ([`registry::closure_gap`]). Deferred consent has its separate selected transition gate.
pub fn stranded(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl DefinitionLookup,
    live: &LiveObligations,
) -> Option<String> {
    if live.is_empty() {
        return None;
    }
    if live.orderbook_custody != 0
        && let Some(reason) = super::native_escrow_capture::orderbook_admission_gap(successor)
    {
        return Some(format!(
            "{} live orderbook custody obligations: {reason}",
            live.orderbook_custody
        ));
    }
    if let Some(reason) = registry::closure_gap(predecessor, successor, published, &live.registered)
    {
        return Some(reason);
    }
    if let Some(reason) = super::native_escrow_capture::closure_gap(
        predecessor,
        successor,
        published,
        &live.captured_escrows,
    ) {
        return Some(reason);
    }
    if let Some(reason) = super::native_amx_capture::closure_gap(
        predecessor,
        successor,
        published,
        &live.captured_amx,
    ) {
        return Some(reason);
    }
    if live.sorafs_authorities != 0 {
        let ids = [
            "iroha.sorafs.authority.propose.v1",
            "iroha.sorafs.authority.attempt.create.v1",
            "iroha.governance.parliament.transition.submit.v1",
        ];
        if let Some(reason) = closure_gap(predecessor, successor, published, &ids, false) {
            return Some(format!(
                "{} live SoraFS authority obligations: {reason}",
                live.sorafs_authorities
            ));
        }
        if predecessor.services_profile != successor.services_profile {
            return Some("live SoraFS authorities retain their exact services profile".into());
        }
    }
    Class::ALL.into_iter().find_map(|class| {
        let count = live.count(class);
        if count == 0 {
            return None;
        }
        let gap = closure_gap(
            predecessor,
            successor,
            published,
            &class.completions(),
            class.invokes_contracts(),
        )?;
        let (kind, awaiting) = class.kind();
        Some(format!(
            "{count} live {kind:?} obligations{awaiting}: {gap}"
        ))
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod content_tests;
