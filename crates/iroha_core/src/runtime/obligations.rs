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
//! A restriction never removes settlement authority (§7.1):
//!
//! - while a definition behind an instruction has admission closed, the instructions that
//!   continue or complete a live obligation still run, because work already admitted continues;
//! - while it is completion-only, the instructions that complete a live obligation still run;
//! - an instruction that starts new work runs under neither;
//! - a change set whose successor changes the bindings or declared dependency closure of a live
//!   obligation's completions, or disables what of them its predecessor executes, lapses at its
//!   effective height. Until captured completion capabilities support rebinding, retaining an
//!   active replacement implementation is insufficient: it has not inherited the original
//!   obligation.
//!
//! **Escrow steps.** Accepting an offer continues an escrow and adds the buyer's exposure. Marking
//! payment sent, releasing, cancelling, opening or resolving a dispute, attesting a condition,
//! drawing down and expiring complete it. Opening one is new work.
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
//! TODO(§7.2): obligations created by contracts register through a kernel syscall with their
//! captured completion closure, retained in `RootSelectionV1::completion_closures`, and complete
//! under it after a rebinding; obligations transfer under their preserved invariants; pending
//! consent (multisig, §8.2), scheduled triggers and migration cursors become further kinds.

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
    let (participation, step) = amx_step(instruction)?;
    participation
        .is_live(
            world.sumeragi_amx.get(),
            world.sumeragi_amx_participant.get().canonical(),
        )
        .then_some(step)
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
    /// Asset escrows and locks in custody.
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
    /// AMX transactions this root coordinates and has not decided.
    amx_decisions: usize,
    /// AMX escrows this root holds as a participant and has not settled.
    amx_settlements: usize,
}

impl LiveObligations {
    /// Live obligations of `world`.
    pub(crate) fn of(world: &crate::state::WorldTransaction<'_, '_>) -> Self {
        Self {
            escrows: world
                .asset_escrows
                .iter()
                .filter(|(_, record)| is_live(record.status))
                .count(),
            ..Self::amx(
                world.sumeragi_amx.get(),
                world.sumeragi_amx_participant.get().canonical(),
            )
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
            amx_decisions: coordinator
                .transactions
                .iter()
                .filter(|entry| entry.decided.is_none())
                .count(),
            amx_settlements: participant.map_or(0, unsettled_escrows),
        }
    }

    /// Whether no obligation is live.
    pub(crate) fn is_empty(&self) -> bool {
        Class::ALL.into_iter().all(|class| self.count(class) == 0)
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
            amx_decisions: decisions,
            amx_settlements: settlements,
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
/// No live obligation has a captured completion capability that dispatch can follow after a
/// rebinding yet. Keep the predecessor closure of each class of live obligations unchanged
/// through every activation, rather than silently treating a newly bound implementation as their
/// settlement authority.
///
/// TODO(§7.2): pending consent, scheduled triggers and migration cursors do not register as
/// obligations yet; `LiveObligations::of` counts none.
pub fn stranded(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl DefinitionLookup,
    live: &LiveObligations,
) -> Option<String> {
    if live.is_empty() {
        return None;
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
