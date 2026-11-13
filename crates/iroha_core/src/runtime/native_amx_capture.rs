//! Selected native AMX captures, shared-tracker cohorts and automatic deadline custody.

use iroha_crypto::Hash;
use iroha_data_model::{
    isi::{
        InstructionBox, error::InstructionExecutionError as InstructionError, sumeragi_amx::*,
        wire_id_of,
    },
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionLookup, RootSelectionV1},
        native_amx::{
            NativeAmxCaptureV1 as Record, NativeAmxKindV1 as Kind, SERVICE, SERVICE_CLOSURE,
        },
        obligation::{CompletionCaptureV1, ObligationId},
    },
    sumeragi_amx::{
        AmxGlobalEntryV1, AmxRecordV1, AmxTransactionV1, AmxVoteV1, NativeAmxParticipantStateV1,
    },
};
use mv::storage::StorageReadOnly as _;

use super::{change_set::PublishedDefinitions, execution::NativeExecution, obligations::Step};
use crate::state::{StateBlock, StateTransaction, WorldReadOnly, WorldTransaction};

/// Execution preserves the original AMX codec refusal; restore/activation only report diagnostics.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// A protocol invariant or selected-context mismatch.
    #[error(transparent)]
    Rejected(InstructionError),
    /// The original proof/codec error, including retained local resource custody.
    #[error(transparent)]
    Amx(iroha_data_model::sumeragi_amx::AmxError),
}
impl Error {
    /// Preserve the automatic finalizer's original codec refusal through its existing AMX boundary.
    pub(crate) fn into_amx(self) -> iroha_data_model::sumeragi_amx::AmxError {
        match self {
            Self::Amx(error) => error,
            Self::Rejected(error) => {
                iroha_data_model::sumeragi_amx::AmxError::Encoding(error.to_string())
            }
        }
    }

    fn into_instruction(self, tx: &mut StateTransaction<'_, '_>) -> InstructionError {
        match self {
            Self::Rejected(error) => error,
            Self::Amx(error) => crate::sumeragi::amx::amx_error(&error, tx),
        }
    }
}

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::Rejected(InstructionError::InvariantViolation(
        format!("native AMX capture: {reason}").into(),
    ))
}

/// Whether the exact compiled native AMX service owns this root's execution.
pub(crate) fn enabled(selection: &RootSelectionV1) -> Result<bool, String> {
    super::release_catalog()
        .map_err(|error| error.to_string())?
        .service_enabled(
            selection.services_profile,
            SERVICE,
            Hash::new(SERVICE_CLOSURE).into(),
        )
        .map_err(|error| error.to_string())
}

fn hash_canonical(domain: &[u8], value: &impl norito::NoritoSerialize) -> Result<[u8; 32], Error> {
    norito::core::classify_decode_attempt(|| {
        let mut codec = None;
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(domain)?;
            norito::core::write_canonical_to_writer(value, writer).map_err(|cause| {
                codec = Some(cause);
                std::io::Error::from(std::io::ErrorKind::Other)
            })
        });
        if let Some(cause) = codec {
            return Err(cause);
        }
        hash.map(Into::into).map_err(norito::Error::Io)
    })
    .map_err(|error| {
        if error.kind() != norito::core::DecodeAttemptErrorKind::Invalid {
            Error::Amx(iroha_data_model::sumeragi_amx::AmxError::Resource(error))
        } else {
            refused(error)
        }
    })
}

fn identity(kind: Kind, tx: [u8; 32]) -> Result<ObligationId, Error> {
    hash_canonical(b"iroha:runtime:native-amx-capture-id:v1\0", &(kind, tx)).map(ObligationId)
}

fn digest(value: &impl norito::NoritoSerialize) -> Result<[u8; 32], Error> {
    hash_canonical(b"iroha:runtime:native-amx-opening:v1\0", value)
}

fn global_opening(world: &impl WorldReadOnly, entry: &AmxGlobalEntryV1) -> Result<[u8; 32], Error> {
    let participants = entry
        .begin
        .participants
        .iter()
        .map(|id| {
            let tracker = &world
                .sumeragi_amx()
                .dataspace(*id)
                .ok_or_else(|| refused("original participant tracker is absent"))?
                .tracker;
            Ok((
                *id,
                tracker.instance,
                tracker.current.network_id,
                tracker.result_scope,
            ))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    digest(&(
        entry.begin.clone(),
        entry.begun_at,
        entry.begun_time_ms,
        entry.original_cadence_ms,
        participants,
    ))
}

fn participant_opening(
    source: &NativeAmxParticipantStateV1,
    tx: &[u8; 32],
) -> Result<[u8; 32], Error> {
    let entry = source
        .participant
        .entry(tx)
        .ok_or_else(|| refused("Yes vote is absent"))?;
    let escrow = source
        .escrows
        .binary_search_by_key(tx, |row| row.tx)
        .ok()
        .and_then(|slot| source.escrows.get(slot))
        .ok_or_else(|| refused("Yes custody is absent"))?;
    if entry.settled.is_some()
        || escrow.settled.is_some()
        || entry.vote != AmxVoteV1::Yes(escrow.effects_hash)
        || escrow.custody != source.custody
        || iroha_data_model::sumeragi_amx::native_transfer_effects_hash(&escrow.leg)
            .map_err(Error::Amx)?
            != escrow.effects_hash
    {
        return Err(refused(
            "Yes vote and exact original monetary custody disagree",
        ));
    }
    digest(&(
        source.participant.dataspace,
        source.participant.global.instance,
        source.participant.global.current.network_id,
        (
            Hash::new(&source.global_genesis),
            Hash::new(&source.global_successor),
            Hash::new(&source.global_chain_label),
        ),
        *tx,
        escrow.effects_hash,
        escrow.custody.clone(),
        entry.begun_at,
        entry.begun_time_ms,
        entry.deadline,
        entry.expires_at_ms,
    ))
}

fn wires(kind: Kind) -> Vec<String> {
    let values = match kind {
        Kind::GlobalDecision => vec![
            wire_id_of::<BeginAmxV1>(),
            wire_id_of::<RelayAmxPreparedV1>(),
            wire_id_of::<ExpireAmxV1>(),
            wire_id_of::<RelayAmxHandoffV1>(),
        ],
        Kind::ParticipantSettlement(_) => vec![
            wire_id_of::<PrepareAmxV1>(),
            wire_id_of::<SettleAmxV1>(),
            wire_id_of::<RelayGlobalAmxHandoffV1>(),
        ],
    };
    let mut values: Vec<_> = values
        .into_iter()
        .map(|wire| wire.expect("AMX wire is registered").to_owned())
        .collect();
    values.sort();
    values
}

// Native AMX performs no contract calls. Exclude unrelated host roots before walking
// dependencies, so they cannot change the default implementation or nested bindings.
fn capture_native(
    canonical: &RootSelectionV1,
    definitions: &impl DefinitionLookup,
    terminal: &[String],
    height: u64,
) -> Result<CompletionCaptureV1, Error> {
    let mut selection = canonical.clone();
    selection.active.retain(|id| {
        !matches!(
            definitions.definition(id),
            Some(DefinitionDescriptorV1::HostProfile(_))
        )
    });
    CompletionCaptureV1::capture(&selection, definitions, terminal, height).map_err(refused)
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use iroha_data_model::runtime::definition::{
        HostProfileDescriptorV1, RestrictionEntryV1, RestrictionV1,
    };

    #[test]
    fn native_amx_capture_excludes_host_roots_and_preserves_native_closure_and_restrictions() {
        let catalog = crate::runtime::release_catalog().unwrap();
        let network = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"AMX capture test")),
        );
        let mut selection = catalog
            .activation(
                crate::runtime::catalog::additions::NATIVE_AMX_CAPTURE.module,
                &catalog.baseline,
                &catalog.definitions,
                network,
                50,
                [1; 32],
            )
            .unwrap()
            .apply(&catalog.baseline, &catalog.definitions)
            .unwrap()
            .selection;
        let mut definitions = catalog.definitions.clone();
        let profile = DefinitionDescriptorV1::HostProfile(HostProfileDescriptorV1 {
            abi_version: 1,
            abi_hash: [49; 32],
            metering: [18; 32],
        });
        let profile_id = profile.id().unwrap();
        definitions.insert(profile_id, profile);
        selection.active.push(profile_id);
        selection.active.sort();
        selection.restrictions.push(RestrictionEntryV1 {
            definition: profile_id,
            restriction: RestrictionV1::Disabled(50),
        });
        selection.restrictions.sort_by_key(|entry| entry.definition);
        for terminal in [
            wires(Kind::GlobalDecision),
            wires(Kind::ParticipantSettlement(
                iroha_model_base::topology::DataSpaceId::new(7),
            )),
            vec![wire_id_of::<PrepareAmxV1>().unwrap().to_owned()],
        ] {
            let capture = capture_native(&selection, &definitions, &terminal, 50).unwrap();
            assert!(capture.host_profiles.is_empty());
            let mut expected =
                CompletionCaptureV1::capture(&selection, &definitions, &terminal, 50).unwrap();
            assert!(expected.host_profiles.contains(&profile_id));
            expected.host_profiles.clear();
            assert_eq!(capture, expected);
            assert!(capture.default_implementation.is_some());
            assert_eq!(capture.terminal_effects().collect::<Vec<_>>(), terminal);
            // An unrelated restricted host cannot grant an exemption to a native terminal.
            let mut restricted = selection.clone();
            restricted.restrictions.push(RestrictionEntryV1 {
                definition: capture.terminal[0].implementation,
                restriction: RestrictionV1::Disabled(50),
            });
            restricted
                .restrictions
                .sort_by_key(|entry| entry.definition);
            assert!(capture_native(&restricted, &definitions, &terminal, 50).is_err());
        }
    }
}

fn context(
    world: &impl WorldReadOnly,
    canonical: &RootSelectionV1,
    capture: &CompletionCaptureV1,
) -> Result<RootSelectionV1, Error> {
    capture
        .execution_selection(
            canonical,
            &PublishedDefinitions(world.runtime_definitions()),
        )
        .map_err(refused)
}

fn ensure_actual(
    tx: &StateTransaction<'_, '_>,
    capture: &CompletionCaptureV1,
) -> Result<(), Error> {
    let expected = context(&*tx.world, tx.world.runtime_selection.canonical(), capture)?;
    let actual = tx.world.runtime_selection.get();
    if (
        actual.kernel,
        actual.commitment_profile,
        actual.services_profile,
    ) != (
        expected.kernel,
        expected.commitment_profile,
        expected.services_profile,
    ) {
        return Err(refused(
            "actual root profiles differ from original AMX custody",
        ));
    }
    // Ordinary execution may also contain unrelated independent additions. Compare every
    // binding and retained definition of this exact capture; a sibling definition outside
    // its closure neither grants authority nor makes the original AMX operation unavailable.
    if let Some(reason) = capture
        .broken_by(
            &expected,
            actual,
            &PublishedDefinitions(&tx.world.runtime_definitions),
        )
        .map_err(refused)?
    {
        return Err(refused(reason));
    }
    Ok(())
}

fn authoritative<'a>(
    world: &'a impl WorldReadOnly,
    kind: Kind,
    tx: [u8; 32],
) -> Result<&'a Record, Error> {
    let opening = match kind {
        Kind::GlobalDecision => {
            let entry = world
                .sumeragi_amx()
                .transaction(&tx)
                .filter(|entry| entry.decided.is_none())
                .ok_or_else(|| refused("undecided Begin is absent"))?;
            global_opening(world, entry)?
        }
        Kind::ParticipantSettlement(dataspace) => {
            let source = world
                .sumeragi_amx_participant()
                .canonical()
                .filter(|source| source.participant.dataspace == dataspace)
                .ok_or_else(|| refused("native participant is absent"))?;
            participant_opening(source, &tx)?
        }
    };
    let record = world
        .runtime_amx_captures()
        .get(&identity(kind, tx)?)
        .ok_or_else(|| refused("live AMX capture is absent"))?;
    if record.kind != kind || record.tx != tx || record.opening != opening {
        return Err(refused(
            "retained capture differs from immutable AMX opening",
        ));
    }
    Ok(record)
}

fn participant_live(world: &impl WorldReadOnly) -> Vec<(Kind, [u8; 32])> {
    world
        .sumeragi_amx_participant()
        .canonical()
        .map_or_else(Vec::new, |source| {
            source
                .participant
                .prepared
                .iter()
                .filter(|entry| entry.settled.is_none() && matches!(entry.vote, AmxVoteV1::Yes(_)))
                .map(|entry| {
                    (
                        Kind::ParticipantSettlement(source.participant.dataspace),
                        entry.tx,
                    )
                })
                .collect()
        })
}

fn live(world: &impl WorldReadOnly) -> Vec<(Kind, [u8; 32])> {
    world
        .sumeragi_amx()
        .transactions
        .iter()
        .filter(|entry| entry.decided.is_none())
        .map(|entry| (Kind::GlobalDecision, entry.begin.tx))
        .chain(participant_live(world))
        .collect()
}

/// Opaque opening prepared before any certificate witness or monetary mutation.
pub(crate) struct PreparedCapture(Option<(ObligationId, Record)>);
impl PreparedCapture {
    /// Publish beside the already authenticated native owner, with no fallible work remaining.
    pub(crate) fn publish(self, world: &mut WorldTransaction<'_, '_>) {
        if let Some((id, record)) = self.0 {
            world.runtime_amx_captures.insert(id, record);
        }
    }
}

fn new_capture(
    tx: &StateTransaction<'_, '_>,
    kind: Kind,
    transaction: [u8; 32],
    opening: [u8; 32],
    cohort: impl IntoIterator<Item = (Kind, [u8; 32])>,
) -> Result<PreparedCapture, Error> {
    let id = identity(kind, transaction)?;
    if tx.world.runtime_amx_captures.get(&id).is_some() {
        return Err(refused("AMX opening already captured"));
    }
    let canonical = tx.world.runtime_selection.canonical();
    let capture = capture_native(
        canonical,
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &wires(kind),
        tx.block_height(),
    )?;
    ensure_actual(tx, &capture)?;
    let expected = context(&*tx.world, canonical, &capture)?;
    for (kind, other) in cohort {
        let retained = authoritative(&*tx.world, kind, other)?;
        if context(&*tx.world, canonical, &retained.capture)? != expected {
            return Err(refused(
                "new AMX work disagrees with a live shared-tracker cohort",
            ));
        }
    }
    Ok(PreparedCapture(Some((
        id,
        Record {
            kind,
            tx: transaction,
            opening,
            capture,
        },
    ))))
}

/// Check creator admission even when Prepare will vote No or close a held decision.
fn check_prepare_admission_checked(tx: &StateTransaction<'_, '_>) -> Result<(), Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(());
    }
    let capture = capture_native(
        tx.world.runtime_selection.canonical(),
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &[wire_id_of::<PrepareAmxV1>()
            .expect("AMX creator is registered")
            .into()],
        tx.block_height(),
    )?;
    ensure_actual(tx, &capture)?;
    // Even a No/held Prepare advances shared participant verification State.
    guard_participant_cohort_checked(tx)
}

/// Capture canonical global decision authority before the Begin write and witness.
fn prepare_begin_checked(
    tx: &StateTransaction<'_, '_>,
    transaction: &AmxTransactionV1,
    height: u64,
    time_ms: u64,
    cadence_ms: u64,
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    let begin = transaction.begin().map_err(Error::Amx)?;
    if tx.world.sumeragi_amx().transaction(&begin.tx).is_some() {
        return Err(refused("Begin already exists"));
    }
    let cohort: Vec<_> = tx
        .world
        .sumeragi_amx()
        .transactions
        .iter()
        .filter(|entry| {
            entry.decided.is_none()
                && entry
                    .begin
                    .participants
                    .iter()
                    .any(|participant| begin.has_participant(*participant))
        })
        .map(|entry| (Kind::GlobalDecision, entry.begin.tx))
        .collect();
    let opening = global_opening(
        &*tx.world,
        &AmxGlobalEntryV1 {
            begin: begin.clone(),
            begun_at: height,
            begun_time_ms: time_ms,
            original_cadence_ms: cadence_ms,
            yes: Vec::new(),
            decided: None,
        },
    )?;
    new_capture(tx, Kind::GlobalDecision, begin.tx, opening, cohort)
}

/// Capture only a verified new Yes candidate, before its witness and debit.
fn prepare_yes_checked(
    tx: &StateTransaction<'_, '_>,
    candidate: &NativeAmxParticipantStateV1,
    transaction: [u8; 32],
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    let kind = Kind::ParticipantSettlement(candidate.participant.dataspace);
    let opening = participant_opening(candidate, &transaction)?;
    new_capture(tx, kind, transaction, opening, participant_live(&*tx.world))
}

/// Check one original native completion before a direct caller can change custody or witnesses.
fn guard_checked(
    tx: &StateTransaction<'_, '_>,
    kind: Kind,
    transaction: [u8; 32],
) -> Result<(), Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(());
    }
    ensure_actual(tx, &authoritative(&*tx.world, kind, transaction)?.capture)
}

/// Exact release identity checked before any decision witness or monetary movement.
pub(crate) struct PreparedCompletion(Option<ObligationId>);
impl PreparedCompletion {
    /// Release only after the corresponding authoritative native mutation succeeds.
    pub(crate) fn publish(self, world: &mut WorldTransaction<'_, '_>) {
        if let Some(id) = self.0 {
            world.runtime_amx_captures.remove(id);
        }
    }
}

/// Check a completion and retain its exact side-table key before native effects.
fn prepare_completion_checked(
    tx: &StateTransaction<'_, '_>,
    kind: Kind,
    transaction: [u8; 32],
) -> Result<PreparedCompletion, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCompletion(None));
    }
    guard_checked(tx, kind, transaction)?;
    Ok(PreparedCompletion(Some(identity(kind, transaction)?)))
}

fn operation_members(
    world: &impl WorldReadOnly,
    instruction: &InstructionBox,
) -> Option<Vec<(Kind, [u8; 32])>> {
    let raw = instruction.as_any();
    if let Some(relay) = raw.downcast_ref::<RelayAmxPreparedV1>() {
        let AmxRecordV1::Prepared(prepared) = &relay.proof.record else {
            return None;
        };
        return Some(
            world
                .sumeragi_amx()
                .transaction(&prepared.tx)
                .filter(|entry| entry.decided.is_none())
                .map(|_| vec![(Kind::GlobalDecision, prepared.tx)])
                .unwrap_or_default(),
        );
    }
    if let Some(expire) = raw.downcast_ref::<ExpireAmxV1>() {
        return Some(
            world
                .sumeragi_amx()
                .transaction(&expire.tx)
                .filter(|entry| entry.decided.is_none())
                .map(|_| vec![(Kind::GlobalDecision, expire.tx)])
                .unwrap_or_default(),
        );
    }
    if let Some(handoff) = raw.downcast_ref::<RelayAmxHandoffV1>() {
        return Some(
            world
                .sumeragi_amx()
                .transactions
                .iter()
                .filter(|entry| {
                    entry.decided.is_none() && entry.begin.has_participant(handoff.dataspace)
                })
                .map(|entry| (Kind::GlobalDecision, entry.begin.tx))
                .collect(),
        );
    }
    if let Some(settle) = raw.downcast_ref::<SettleAmxV1>() {
        let AmxRecordV1::Decision(decision) = &settle.decision.record else {
            return None;
        };
        return Some(
            participant_live(world)
                .into_iter()
                .filter(|(kind, tx)| {
                    *kind == Kind::ParticipantSettlement(settle.dataspace) && *tx == decision.tx
                })
                .collect(),
        );
    }
    if let Some(handoff) = raw.downcast_ref::<RelayGlobalAmxHandoffV1>() {
        return Some(
            participant_live(world)
                .into_iter()
                .filter(|(kind, _)| *kind == Kind::ParticipantSettlement(handoff.dataspace))
                .collect(),
        );
    }
    None
}

/// Whether the instruction changes native AMX coordinator or participant State.
fn is_amx_instruction(instruction: &InstructionBox) -> bool {
    let raw = instruction.as_any();
    raw.is::<BeginAmxV1>()
        || raw.is::<PrepareAmxV1>()
        || raw.is::<RegisterAmxDataspaceV1>()
        || raw.is::<RegisterAmxParticipantV1>()
        || raw.is::<ExpireAmxV1>()
        || raw.is::<RelayAmxPreparedV1>()
        || raw.is::<RelayAmxHandoffV1>()
        || raw.is::<SettleAmxV1>()
        || raw.is::<RelayGlobalAmxHandoffV1>()
}

/// Neither an enclosing native operation nor a user registration can authorize new AMX work.
fn ordinary_instruction(
    tx: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<(), Error> {
    let canonical = tx.world.runtime_selection.canonical();
    super::resolve(instruction, canonical, tx.block_height(), || None).map_err(Error::Rejected)?;
    let wire = iroha_data_model::isi::instruction_wire_id(instruction)
        .ok_or_else(|| refused("AMX operation has no registered wire identity"))?;
    let capture = capture_native(
        canonical,
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &[wire.to_owned()],
        tx.block_height(),
    )?;
    ensure_actual(tx, &capture)
}

/// Select a single context only after checking every authoritative member of a shared operation.
fn prepare_execution_checked(
    tx: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<Option<NativeExecution>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    let Some(members) = operation_members(&*tx.world, instruction) else {
        if is_amx_instruction(instruction) {
            ordinary_instruction(tx, instruction)?;
        }
        return Ok(None);
    };
    if members.is_empty() {
        ordinary_instruction(tx, instruction)?;
        return Ok(None);
    }
    let mut agreed = None;
    for (kind, transaction) in members {
        let record = authoritative(&*tx.world, kind, transaction)?;
        let selected = context(
            &*tx.world,
            tx.world.runtime_selection.canonical(),
            &record.capture,
        )?;
        if let Some((expected, _)) = &agreed {
            if expected != &selected {
                return Err(refused(
                    "shared AMX operation has differing original contexts",
                ));
            }
        } else {
            agreed = Some((selected, record.capture.clone()));
        }
    }
    Ok(agreed.map(|(_, capture)| NativeExecution {
        capture,
        step: Step::Complete,
    }))
}

/// Every participant mutation verifies all live Yes owners, even a nonterminal held proof.
fn guard_participant_cohort_checked(tx: &StateTransaction<'_, '_>) -> Result<(), Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(());
    }
    for (kind, transaction) in participant_live(&*tx.world) {
        guard_checked(tx, kind, transaction)?;
    }
    Ok(())
}

/// A foreign handoff updates the exact shared tracker of all decisions naming its data space.
fn guard_global_handoff_checked(
    tx: &StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<(), Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(());
    }
    for entry in tx
        .world
        .sumeragi_amx()
        .transactions
        .iter()
        .filter(|entry| entry.decided.is_none() && entry.begin.has_participant(dataspace))
    {
        guard_checked(tx, Kind::GlobalDecision, entry.begin.tx)?;
    }
    Ok(())
}

/// Checked due identities retained across the service-owned automatic expiry step.
pub(crate) struct PreparedDeadline(Vec<ObligationId>);
impl PreparedDeadline {
    /// Remove due captures only after every original decision witness was accepted.
    pub(crate) fn publish(self, block: &mut StateBlock<'_>) {
        for id in self.0 {
            block.world.runtime_amx_captures.remove(id);
        }
    }
}

/// The immutable service owns automatic expiry; each due context is checked independently.
pub(crate) fn prepare_deadline(
    block: &StateBlock<'_>,
    height: u64,
    time_ms: u64,
) -> Result<PreparedDeadline, Error> {
    let world = block.world();
    if !enabled(world.runtime_selection()).map_err(refused)? {
        return Ok(PreparedDeadline(Vec::new()));
    }
    let mut due = Vec::new();
    for entry in world
        .sumeragi_amx()
        .transactions
        .iter()
        .filter(|entry| entry.decided.is_none() && entry.begin.is_due(height, time_ms))
    {
        let capture = authoritative(world, Kind::GlobalDecision, entry.begin.tx)?;
        // No arbitrary member lends its scope to the batch: disabled/unretained definitions
        // refuse before the first witness, while the pinned service owns the fixed finalizer.
        let retained = context(world, world.runtime_selection(), &capture.capture)?;
        if retained.restrictions.iter().any(|entry| {
            matches!(entry.restriction,
            iroha_data_model::runtime::definition::RestrictionV1::Disabled(from) if from <= height)
        }) {
            return Err(refused(
                "automatic AMX expiry depends on a disabled original definition",
            ));
        }
        due.push(identity(Kind::GlobalDecision, entry.begin.tx)?);
    }
    Ok(PreparedDeadline(due))
}

/// No selected AMX owner may be restored without exactly its original live capture.
pub(crate) fn validate_persistence(world: &impl WorldReadOnly) -> Result<(), Error> {
    if !enabled(world.runtime_selection()).map_err(refused)? {
        return Ok(());
    }
    let owners = live(world);
    if owners.len() != world.runtime_amx_captures().iter().count() {
        return Err(refused("AMX capture count differs from live owners"));
    }
    for (kind, tx) in &owners {
        let capture = authoritative(world, *kind, *tx)?;
        context(world, world.runtime_selection(), &capture.capture)?;
    }
    // Every unresolved native escrow must still have its corresponding Yes owner.
    if let Some(participant) = world.sumeragi_amx_participant().canonical() {
        for escrow in participant
            .escrows
            .iter()
            .filter(|row| row.settled.is_none())
        {
            authoritative(
                world,
                Kind::ParticipantSettlement(participant.participant.dataspace),
                escrow.tx,
            )?;
        }
    }
    for (id, record) in world.runtime_amx_captures().iter() {
        if *id != identity(record.kind, record.tx)? || !owners.contains(&(record.kind, record.tx)) {
            return Err(refused("orphaned AMX capture"));
        }
    }
    // One immutable context identity per shared tracker avoids a quadratic pairwise
    // scan or retaining a full selection for every participant of every Begin.
    let mut global_cohorts = std::collections::BTreeMap::new();
    let mut participant_cohorts = std::collections::BTreeMap::new();
    for (kind, tx) in owners {
        let record = authoritative(world, kind, tx)?;
        let context_id = context(world, world.runtime_selection(), &record.capture)?
            .id()
            .map_err(refused)?;
        let cohort = match kind {
            Kind::GlobalDecision => {
                let begin = &world
                    .sumeragi_amx()
                    .transaction(&tx)
                    .expect("checked Begin")
                    .begin;
                for participant in &begin.participants {
                    if global_cohorts
                        .insert(*participant, context_id)
                        .is_some_and(|prior| prior != context_id)
                    {
                        return Err(refused("restored global AMX cohort contexts disagree"));
                    }
                }
                continue;
            }
            Kind::ParticipantSettlement(dataspace) => participant_cohorts.entry(dataspace),
        };
        match cohort {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(context_id);
            }
            std::collections::btree_map::Entry::Occupied(slot) if *slot.get() != context_id => {
                return Err(refused("restored participant AMX cohort contexts disagree"));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Activation and retirement never manufacture consent for earlier AMX work.
pub(crate) fn check_transition(
    world: &impl WorldReadOnly,
    before: &RootSelectionV1,
    after: &RootSelectionV1,
) -> Result<(), String> {
    let was = enabled(before)?;
    let next = enabled(after)?;
    if was {
        validate_persistence(world).map_err(|error| error.to_string())?;
    }
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let storage = catalog
        .additions
        .iter()
        .find(|addition| addition.module == super::catalog::additions::NATIVE_AMX_CAPTURE.module)
        .and_then(|addition| addition.storage.get("world.runtime_amx_captures"));
    if let Some(storage) = storage {
        if next != after.installed_storage.binary_search(storage).is_ok() {
            return Err("native AMX service and capture table must be selected together".into());
        }
    } else if was || next {
        return Err("native AMX service has no compiled capture table".into());
    }
    if was != next
        && (!live(world).is_empty()
            || world.runtime_amx_captures().iter().next().is_some()
            || world
                .sumeragi_amx_participant()
                .canonical()
                .is_some_and(|source| source.escrows.iter().any(|row| row.settled.is_none())))
    {
        return Err(
            "native AMX service transition requires prior decisions and Yes custody to drain"
                .into(),
        );
    }
    Ok(())
}

/// Group live original captures for ordinary activation and definition retirement checks.
pub(crate) fn live_captures(world: &WorldTransaction<'_, '_>) -> Vec<(CompletionCaptureV1, usize)> {
    if enabled(world.runtime_selection.canonical()).ok() != Some(true) {
        return Vec::new();
    }
    let mut captures = std::collections::BTreeMap::new();
    for (kind, tx) in live(world) {
        if let Ok(record) = authoritative(world, kind, tx) {
            *captures.entry(record.capture.clone()).or_default() += 1;
        }
    }
    captures.into_iter().collect()
}

/// Retain each original AMX graph while ordinary instruction bindings may move.
pub(crate) fn closure_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
    captures: &[(CompletionCaptureV1, usize)],
) -> Option<String> {
    captures.iter().find_map(|(capture, count)| {
        let context = match capture.execution_selection(successor, published) {
            Ok(context) => context,
            Err(error) => return Some(format!("{count} live native AMX obligations: {error}")),
        };
        let reason = match capture.broken_by(predecessor, &context, published) {
            Ok(reason) => reason,
            Err(error) => Some(error.to_string()),
        }?;
        Some(format!("{count} live native AMX obligations: {reason}"))
    })
}

/// Prepare original canonical Begin custody before mutation or witness output.
pub(crate) fn prepare_begin(
    tx: &mut StateTransaction<'_, '_>,
    transaction: &AmxTransactionV1,
    height: u64,
    time_ms: u64,
    cadence_ms: u64,
) -> Result<PreparedCapture, InstructionError> {
    prepare_begin_checked(tx, transaction, height, time_ms, cadence_ms)
        .map_err(|error| error.into_instruction(tx))
}
/// Refuse newly admitted participant work before shared tracker or monetary effects.
pub(crate) fn check_prepare_admission(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionError> {
    check_prepare_admission_checked(tx).map_err(|error| error.into_instruction(tx))
}
/// Capture only the candidate's authenticated, funded Yes before its witness or debit.
pub(crate) fn prepare_yes(
    tx: &mut StateTransaction<'_, '_>,
    candidate: &NativeAmxParticipantStateV1,
    transaction: [u8; 32],
) -> Result<PreparedCapture, InstructionError> {
    prepare_yes_checked(tx, candidate, transaction).map_err(|error| error.into_instruction(tx))
}
/// Check the original identity and execution context before a native monetary effect.
pub(crate) fn guard(
    tx: &mut StateTransaction<'_, '_>,
    kind: Kind,
    transaction: [u8; 32],
) -> Result<(), InstructionError> {
    guard_checked(tx, kind, transaction).map_err(|error| error.into_instruction(tx))
}
/// Prepare the infallible capture removal before terminal output or monetary effects.
pub(crate) fn prepare_completion(
    tx: &mut StateTransaction<'_, '_>,
    kind: Kind,
    transaction: [u8; 32],
) -> Result<PreparedCompletion, InstructionError> {
    prepare_completion_checked(tx, kind, transaction).map_err(|error| error.into_instruction(tx))
}
/// Select a context only after every authoritative member agrees; keep local refusals outside protocol verdicts.
pub(crate) fn prepare_execution(
    tx: &mut StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<Option<NativeExecution>, InstructionError> {
    prepare_execution_checked(tx, instruction).map_err(|error| error.into_instruction(tx))
}
/// Guard every unresolved Yes before any update of the shared global tracker.
pub(crate) fn guard_participant_cohort(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionError> {
    guard_participant_cohort_checked(tx).map_err(|error| error.into_instruction(tx))
}
/// Guard all unresolved global decisions using the named participant's shared tracker.
pub(crate) fn guard_global_handoff(
    tx: &mut StateTransaction<'_, '_>,
    dataspace: iroha_model_base::topology::DataSpaceId,
) -> Result<(), InstructionError> {
    guard_global_handoff_checked(tx, dataspace).map_err(|error| error.into_instruction(tx))
}
