//! Selected destination reservations and source exports retain their original completion graphs.

use iroha_crypto::Hash;
use iroha_data_model::{
    game_partition::{GAME_PARTITION_MAX_RECEIPTS, GAME_PARTITION_MAX_UNITS, GamePartitionStateV1},
    isi::{InstructionBox, error::InstructionExecutionError as Error, game_partition::*},
    runtime::{
        definition::{DefinitionLookup, RootSelectionV1},
        native_game::{
            NativeGameRoleV1 as Role, NativeGameTransferV1 as Record, SERVICE, SERVICE_CLOSURE,
        },
        obligation::CompletionCaptureV1,
    },
};
use mv::storage::StorageReadOnly as _;

use super::{change_set::PublishedDefinitions, execution::NativeExecution, obligations::Step};
use crate::state::{StateTransaction, WorldReadOnly, WorldTransaction};

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("native game capture: {reason}").into())
}

/// Whether this root selects the immutable reservation and final-ACK service.
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

fn wires(role: Role) -> Vec<String> {
    match role {
        Role::Destination => vec![ImportReservedGameUnitV1::WIRE_ID.into(), CancelReservedGameUnitImportV1::WIRE_ID.into()],
        Role::Source => vec![ExportReservedGameUnitV1::WIRE_ID.into(), CancelGameUnitExportRequestV1::WIRE_ID.into(), AcknowledgeGameUnitImportV1::WIRE_ID.into()],
    }
}

fn ensure_execution(
    world: &WorldTransaction<'_, '_>,
    capture: &CompletionCaptureV1,
) -> Result<(), Error> {
    let lookup = PublishedDefinitions(&world.runtime_definitions);
    let expected = capture
        .execution_selection(world.runtime_selection.canonical(), &lookup)
        .map_err(refused)?;
    if let Some(reason) = capture
        .broken_by(&expected, world.runtime_selection.get(), &lookup)
        .map_err(refused)?
    {
        return Err(refused(reason));
    }
    Ok(())
}

/// Prepare native-only custody before either capacity reservation or irreversible source removal.
pub(crate) fn prepare(
    tx: &StateTransaction<'_, '_>,
    role: Role,
) -> Result<CompletionCaptureV1, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Err(refused(
            "destination-reserved transfer service is not selected",
        ));
    }
    let mut capture = CompletionCaptureV1::capture(
        tx.world.runtime_selection.canonical(),
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &wires(role),
        tx.block_height(),
    )
    .map_err(refused)?;
    // Reservation, proof validation and ACK perform no contract calls. Retaining unrelated
    // host profiles would give this native completion authority it does not need.
    capture.host_profiles.clear();
    ensure_execution(&tx.world, &capture)?;
    Ok(capture)
}

/// Verify the native handler is running the exact original completion before any writes.
pub(crate) fn guard(tx: &StateTransaction<'_, '_>, row: &Record) -> Result<(), Error> {
    row.validate().map_err(refused)?;
    let capture = row
        .capture
        .as_ref()
        .ok_or_else(|| refused("transfer has already completed"))?;
    ensure_execution(&tx.world, capture)
}

/// Select an admitted reservation/import or source ACK before current-binding authorization.
pub(crate) fn prepare_execution(
    tx: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<Option<NativeExecution>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    let (id, role) = if let Some(import) = instruction
        .as_any()
        .downcast_ref::<ImportReservedGameUnitV1>()
    {
        (
            import.proof.record.reservation.transfer_id(),
            Role::Destination,
        )
    } else if let Some(ack) = instruction
        .as_any()
        .downcast_ref::<AcknowledgeGameUnitImportV1>()
    {
        (ack.proof.record.transfer_id, Role::Source)
    } else {
        return Ok(None);
    };
    let Some(row) = tx.world.runtime_game_transfers.get(&id) else {
        return Ok(None);
    };
    row.validate().map_err(refused)?;
    if row.role != role {
        return Err(refused("completion addresses the other root's custody"));
    }
    Ok(row.capture.as_ref().map(|capture| NativeExecution {
        capture: capture.clone(),
        step: Step::Complete,
    }))
}

/// Existing destinations reserve both active-unit and identity capacity until actual import.
/// Every selected partition mutation must preserve those commitments.
pub(crate) fn check_partition_capacity(
    world: &impl WorldReadOnly,
    candidate: &GamePartitionStateV1,
) -> Result<(), Error> {
    if !enabled(world.runtime_selection()).map_err(refused)? {
        return Ok(());
    }
    let reservations: Vec<_> = world
        .runtime_game_transfers()
        .iter()
        .filter(|(_, row)| {
            row.role == Role::Destination
                && row.capture.is_some()
                && row.intent.world_id == candidate.world_id
        })
        .map(|(_, row)| &row.intent.unit)
        .collect();
    if candidate.units.len() + reservations.len() > GAME_PARTITION_MAX_UNITS {
        return Err(refused(
            "active capacity is held by original import reservations",
        ));
    }
    let unknown = reservations
        .iter()
        .filter(|unit| {
            !candidate
                .known_units
                .iter()
                .any(|known| known.unit_id == unit.unit_id)
        })
        .count();
    if candidate.known_units.len() + unknown > GAME_PARTITION_MAX_RECEIPTS {
        return Err(refused(
            "identity capacity is held by original import reservations",
        ));
    }
    Ok(())
}

/// Validate every selected custody row and its authentic local correspondence during restore.
pub(crate) fn validate_persistence(world: &impl WorldReadOnly) -> Result<(), Error> {
    let selected = enabled(world.runtime_selection()).map_err(refused)?;
    let mut counts = std::collections::BTreeMap::<Hash, usize>::new();
    let mut reservations = std::collections::BTreeSet::new();
    for (id, row) in world.runtime_game_transfers().iter() {
        if !selected {
            return Err(refused(
                "transfer custody exists without its selected service",
            ));
        }
        row.validate().map_err(refused)?;
        if *id != row.intent.transfer_id() {
            return Err(refused("transfer key differs from original reservation"));
        }
        let partition = world
            .game_partitions()
            .get(&row.intent.world_id)
            .ok_or_else(|| refused("transfer has no authoritative partition"))?;
        let r = &row.intent;
        let (local, remote) = match row.role {
            Role::Source => (r.source, r.destination),
            Role::Destination => (r.destination, r.source),
        };
        if local != partition.local_instance
            || r.kernel_hash != partition.kernel_hash
            || !partition
                .peers
                .iter()
                .any(|peer| peer.tracker.instance == remote)
        {
            return Err(refused(
                "transfer differs from local world, kernel or registered peer",
            ));
        }
        let count = counts.entry(r.world_id).or_default();
        *count += 1;
        if *count > GAME_PARTITION_MAX_RECEIPTS {
            return Err(refused("transfer proof retention bound"));
        }
        match row.role {
            Role::Source => {
                let export = &row.export.as_ref().expect("validated source shape").export;
                if !partition.exports.contains(export) {
                    return Err(refused("source transfer lost its permanent tombstone"));
                }
            }
            Role::Destination if row.capture.is_some() => {
                if !reservations.insert((r.world_id, r.unit.unit_id))
                    || partition
                        .units
                        .iter()
                        .any(|unit| unit.unit_id == r.unit.unit_id)
                    || partition.known_units.iter().any(|known| {
                        known.unit_id == r.unit.unit_id && known.sequence >= r.unit.sequence
                    })
                {
                    return Err(refused(
                        "pending reservation conflicts with original unit custody",
                    ));
                }
            }
            Role::Destination => {
                if !partition.imported.contains(
                    &row.export
                        .as_ref()
                        .expect("validated import shape")
                        .export
                        .receipt_id(),
                ) {
                    return Err(refused(
                        "completed reservation lost its permanent import receipt",
                    ));
                }
            }
        }
        if let Some(capture) = &row.capture {
            if !capture.host_profiles.is_empty() {
                return Err(refused("native transfer captures unrelated host authority"));
            }
            capture
                .execution_selection(
                    world.runtime_selection(),
                    &PublishedDefinitions(world.runtime_definitions()),
                )
                .map_err(refused)?;
        }
    }
    for (_, partition) in world.game_partitions().iter() {
        check_partition_capacity(world, partition)?;
    }
    Ok(())
}

/// Original native closures of still-pending destinations and source acknowledgements.
pub(crate) fn live_captures(world: &WorldTransaction<'_, '_>) -> Vec<(CompletionCaptureV1, usize)> {
    let mut captures = std::collections::BTreeMap::new();
    for (_, row) in world.runtime_game_transfers.iter() {
        if let Some(capture) = &row.capture {
            *captures.entry(capture.clone()).or_default() += 1;
        }
    }
    captures.into_iter().collect()
}

/// Reject removing any completion context until the original certified terminal transition.
pub(crate) fn closure_gap(
    before: &RootSelectionV1,
    after: &RootSelectionV1,
    definitions: &impl DefinitionLookup,
    captures: &[(CompletionCaptureV1, usize)],
) -> Option<String> {
    captures.iter().find_map(|(capture, count)| {
        let context = match capture.execution_selection(after, definitions) {
            Ok(context) => context,
            Err(error) => return Some(format!("{count} pending native game transfers: {error}")),
        };
        match capture.broken_by(before, &context, definitions) {
            Ok(Some(error)) => Some(format!("{count} pending native game transfers: {error}")),
            Err(error) => Some(error.to_string()),
            Ok(None) => None,
        }
    })
}

/// Activation cannot adopt unproved old transfers; retirement cannot discard original custody.
pub(crate) fn check_transition(
    world: &impl WorldReadOnly,
    before: &RootSelectionV1,
    after: &RootSelectionV1,
) -> Result<(), String> {
    let before_enabled = enabled(before)?;
    let after_enabled = enabled(after)?;
    if before_enabled {
        validate_persistence(world).map_err(|error| error.to_string())?;
    }
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let storage = catalog
        .additions
        .iter()
        .find(|a| a.module == super::catalog::additions::NATIVE_GAME_CAPTURE.module)
        .and_then(|a| a.storage.get("world.runtime_game_transfers"));
    if storage.is_none_or(|storage| {
        after_enabled != after.installed_storage.binary_search(storage).is_ok()
    }) {
        return Err("native game service and transfer table must be selected together".into());
    }
    if !before_enabled
        && after_enabled
        && world
            .game_partitions()
            .iter()
            .any(|(_, partition)| !partition.exports.is_empty() || !partition.imported.is_empty())
    {
        return Err(
            "native game activation cannot invent reservations for earlier transfers".into(),
        );
    }
    if before_enabled && !after_enabled && world.runtime_game_transfers().iter().next().is_some() {
        return Err(
            "native game transfer proof/tombstone storage remains authoritative after completion"
                .into(),
        );
    }
    for (_, row) in world.runtime_game_transfers().iter() {
        if let Some(capture) = &row.capture {
            let context = capture
                .execution_selection(after, &PublishedDefinitions(world.runtime_definitions()))
                .map_err(|error| error.to_string())?;
            if let Some(reason) = capture
                .broken_by(
                    before,
                    &context,
                    &PublishedDefinitions(world.runtime_definitions()),
                )
                .map_err(|error| error.to_string())?
            {
                return Err(reason);
            }
        }
    }
    Ok(())
}
