//! Selected, kernel-owned native escrow captures and exact custody lifetime.

use iroha_crypto::Hash;
use iroha_data_model::{
    escrow::{AssetEscrowKind, AssetEscrowRecord, EscrowId},
    isi::{InstructionBox, error::InstructionExecutionError as Error, wire_id_of},
    runtime::{
        definition::RootSelectionV1,
        native_escrow::{
            NativeEscrowCaptureV1 as Record, NativeEscrowKindV1 as Kind, SERVICE, SERVICE_CLOSURE,
        },
        obligation::CompletionCaptureV1,
    },
};
use mv::storage::StorageReadOnly as _;

use super::{change_set::PublishedDefinitions, execution::NativeExecution, obligations::is_live};
use crate::state::{StateTransaction, WorldReadOnly, WorldTransaction};

fn refused(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("native escrow capture: {reason}").into())
}

/// Whether this exact native custody service is selected.
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

/// Custody identity whose fields cannot change during progress or terminal settlement.
fn opening(record: &AssetEscrowRecord) -> Result<[u8; 32], Error> {
    let recipient = (record.kind != AssetEscrowKind::Marketplace).then(|| record.buyer.clone());
    let conditions: Vec<_> = record
        .conditions
        .iter()
        .map(|state| state.condition.clone())
        .collect();
    let bytes = norito::encode_canonical(&(
        record.id,
        record.seller.clone(),
        record.asset_definition.clone(),
        record.amount.clone(),
        record.custody.clone(),
        record.kind,
        record.release_authority.clone(),
        record.expires_at_ms,
        record.created_at_ms,
        recipient,
        conditions,
    ))
    .map_err(refused)?;
    Ok(Hash::new_from_chunks(&[b"iroha:runtime:native-escrow-opening:v1\0", &bytes]).into())
}

fn matches_kind(kind: Kind, record: &AssetEscrowRecord) -> bool {
    use iroha_data_model::{
        escrow::control::is_reserved_controlled_asset_lock_id_v1,
        sorafs::orderbook::{
            is_orderbook_order_escrow_id_v1, is_orderbook_settlement_escrow_id_v1,
        },
    };
    let controlled = is_reserved_controlled_asset_lock_id_v1(&record.id);
    let orderbook = is_orderbook_order_escrow_id_v1(&record.id)
        || is_orderbook_settlement_escrow_id_v1(&record.id);
    match kind {
        Kind::Marketplace => {
            record.kind == AssetEscrowKind::Marketplace && !controlled && !orderbook
        }
        Kind::Conditional => {
            record.kind == AssetEscrowKind::Conditional && !controlled && !orderbook
        }
        Kind::Lock => record.kind == AssetEscrowKind::Lock && !controlled && !orderbook,
        Kind::ControlledLock => record.kind == AssetEscrowKind::Lock && controlled,
        Kind::OrderbookLock => record.kind == AssetEscrowKind::Lock && orderbook,
    }
}

fn wires(kind: Kind) -> Vec<String> {
    use iroha_data_model::isi::{escrow::*, escrow_control::*, sorafs::*};
    let wires = match kind {
        Kind::Marketplace => vec![
            wire_id_of::<OpenAssetEscrow>(),
            wire_id_of::<AcceptAssetEscrow>(),
            wire_id_of::<MarkEscrowPaymentSent>(),
            wire_id_of::<ReleaseAssetEscrow>(),
            wire_id_of::<CancelAssetEscrow>(),
            wire_id_of::<OpenEscrowDispute>(),
            wire_id_of::<ResolveEscrowDispute>(),
        ],
        Kind::Lock => vec![
            wire_id_of::<OpenAssetLock>(),
            wire_id_of::<DrawdownAssetLock>(),
            wire_id_of::<CancelAssetLock>(),
            wire_id_of::<ExpireAssetLock>(),
        ],
        Kind::Conditional => vec![
            wire_id_of::<OpenConditionalEscrow>(),
            wire_id_of::<AttestEscrowCondition>(),
            wire_id_of::<ExpireConditionalEscrow>(),
        ],
        Kind::ControlledLock => vec![
            wire_id_of::<OpenControlledAssetLockV1>(),
            wire_id_of::<DrawdownControlledAssetLockV1>(),
            wire_id_of::<CancelControlledAssetLockV1>(),
            wire_id_of::<ExpireControlledAssetLockV1>(),
        ],
        Kind::OrderbookLock => vec![
            wire_id_of::<SubmitSorafsOrderbookOrder>(),
            wire_id_of::<CancelSorafsOrderbookOrder>(),
            wire_id_of::<MatchSorafsOrderbook>(),
            wire_id_of::<MaintainSorafsOrderbook>(),
            wire_id_of::<RecordSorafsOrderbookSettlementReceipt>(),
        ],
    };
    let mut wires: Vec<_> = wires
        .into_iter()
        .map(|wire| wire.expect("native escrow wire is registered").to_owned())
        .collect();
    wires.sort();
    wires.dedup();
    wires
}

/// Verify actual execution, including an enclosing registered scope, before moving custody.
fn ensure_execution(
    world: &WorldTransaction<'_, '_>,
    capture: &CompletionCaptureV1,
) -> Result<(), Error> {
    let lookup = PublishedDefinitions(&world.runtime_definitions);
    let expected = capture
        .execution_selection(world.runtime_selection.canonical(), &lookup)
        .map_err(refused)?;
    let actual = world.runtime_selection.get();
    if (
        actual.kernel,
        actual.commitment_profile,
        actual.services_profile,
    ) != (
        expected.kernel,
        expected.commitment_profile,
        expected.services_profile,
    ) {
        return Err(refused("actual root profiles differ from retained custody"));
    }
    if let Some(reason) = capture
        .broken_by(&expected, actual, &lookup)
        .map_err(refused)?
    {
        return Err(refused(reason));
    }
    Ok(())
}

/// Capture prepared before any debit or custody-account creation; only native owners construct it.
pub(crate) struct PreparedCapture(Option<Record>);

/// Preserve the inactive opening path without constructing an extra candidate row.
pub(crate) fn prepare_opening_from(
    tx: &StateTransaction<'_, '_>,
    kind: Kind,
    row: impl FnOnce() -> AssetEscrowRecord,
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    prepare_opening(tx, &row(), kind)
}

/// Preserve inactive partition construction while checking new selected custody before debit.
pub(crate) fn prepare_child_from(
    tx: &StateTransaction<'_, '_>,
    parent: &EscrowId,
    child: impl FnOnce() -> AssetEscrowRecord,
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    prepare_child(tx, parent, &child())
}

/// Capture canonical semantics before new funding, and reject a different enclosing execution.
pub(crate) fn prepare_opening(
    tx: &StateTransaction<'_, '_>,
    row: &AssetEscrowRecord,
    kind: Kind,
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    if !is_live(row.status)
        || !matches_kind(kind, row)
        || tx.world.asset_escrows.get(&row.id).is_some()
        || tx.world.runtime_escrow_captures.get(&row.id).is_some()
    {
        return Err(refused("opening does not identify new native custody"));
    }
    let capture = CompletionCaptureV1::capture(
        tx.world.runtime_selection.canonical(),
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &wires(kind),
        tx.block_height(),
    )
    .map_err(refused)?;
    crate::smartcontracts::ivm::host_profile::ensure_new_capture(
        tx.world(),
        &capture,
        tx.block_height(),
    )?;
    ensure_execution(&tx.world, &capture)?;
    Ok(PreparedCapture(Some(Record {
        kind,
        opening: opening(row)?,
        capture,
    })))
}

/// A child of a retained orderbook lock inherits its parent's closure before funds partition.
pub(crate) fn prepare_child(
    tx: &StateTransaction<'_, '_>,
    parent: &EscrowId,
    child: &AssetEscrowRecord,
) -> Result<PreparedCapture, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(PreparedCapture(None));
    }
    guard_existing(tx, parent)?;
    // Partitioning adds a new provider's exposure. A registered terminal effect cannot
    // lend completion-only admission to the outer matcher that creates this child.
    CompletionCaptureV1::capture(
        tx.world.runtime_selection.canonical(),
        &PublishedDefinitions(&tx.world.runtime_definitions),
        &[
            wire_id_of::<iroha_data_model::isi::sorafs::MatchSorafsOrderbook>()
                .expect("native orderbook matcher is registered")
                .into(),
        ],
        tx.block_height(),
    )
    .map_err(refused)?;
    let retained = tx
        .world
        .runtime_escrow_captures
        .get(parent)
        .ok_or_else(|| refused("parent capture is absent"))?;
    if retained.kind != Kind::OrderbookLock
        || !matches_kind(Kind::OrderbookLock, child)
        || !is_live(child.status)
        || tx.world.asset_escrows.get(&child.id).is_some()
        || tx.world.runtime_escrow_captures.get(&child.id).is_some()
    {
        return Err(refused("child is not new orderbook custody"));
    }
    crate::smartcontracts::ivm::host_profile::ensure_new_capture(
        tx.world(),
        &retained.capture,
        tx.block_height(),
    )?;
    Ok(PreparedCapture(Some(Record {
        kind: retained.kind,
        opening: opening(child)?,
        capture: retained.capture.clone(),
    })))
}

/// Check retained authoritative identity and actual bindings before any native custody movement.
pub(crate) fn guard_existing(tx: &StateTransaction<'_, '_>, id: &EscrowId) -> Result<(), Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(());
    }
    let row = tx
        .world
        .asset_escrows
        .get(id)
        .ok_or_else(|| refused("custody row is absent"))?;
    let record = tx
        .world
        .runtime_escrow_captures
        .get(id)
        .ok_or_else(|| refused("live custody capture is absent"))?;
    if !is_live(row.status) || !matches_kind(record.kind, row) || record.opening != opening(row)? {
        return Err(refused(
            "retained capture differs from authoritative custody",
        ));
    }
    ensure_execution(&tx.world, &record.capture)
}

/// Prepare scoped selection before permissions and native dispatch for a direct escrow step.
pub(crate) fn prepare_execution(
    tx: &StateTransaction<'_, '_>,
    instruction: &InstructionBox,
) -> Result<Option<NativeExecution>, Error> {
    if !enabled(tx.world.runtime_selection.canonical()).map_err(refused)? {
        return Ok(None);
    }
    let Some((id, step)) = super::obligations::escrow_step(instruction) else {
        return Ok(None);
    };
    let Some(row) = tx
        .world
        .asset_escrows
        .get(&id)
        .filter(|row| is_live(row.status))
    else {
        return Ok(None);
    };
    let record = tx
        .world
        .runtime_escrow_captures
        .get(&id)
        .ok_or_else(|| refused("live custody capture is absent"))?;
    if record.opening != opening(row)? || !matches_kind(record.kind, row) {
        return Err(refused(
            "retained capture differs from authoritative custody",
        ));
    }
    // An orderbook row has no generic terminal corridor. Its native parent operation stays pinned.
    if record.kind == Kind::OrderbookLock {
        return Err(refused("orderbook custody requires its native owner"));
    }
    let wire = iroha_data_model::isi::instruction_wire_id(instruction)
        .ok_or_else(|| refused("escrow step has no wire identity"))?;
    if !wires(record.kind).iter().any(|allowed| allowed == wire) {
        return Err(refused("escrow step belongs to another native owner"));
    }
    Ok(Some(NativeExecution {
        capture: record.capture.clone(),
        step,
    }))
}

/// Checked side-table update, applied only after the authoritative row's fallible preparation.
pub(crate) enum PreparedWrite {
    /// Frozen inactive behavior, or a still-live existing capture.
    Keep,
    /// New custody's prepared capture.
    Insert(EscrowId, Record),
    /// Authoritative terminal status releases the retained closure.
    Remove(EscrowId),
}

impl PreparedWrite {
    /// Publish alongside the authoritative row and derived indexes, with no fallible work left.
    pub(crate) fn apply(self, world: &mut WorldTransaction<'_, '_>) {
        match self {
            Self::Keep => {}
            Self::Insert(id, capture) => {
                world.runtime_escrow_captures.insert(id, capture);
            }
            Self::Remove(id) => {
                world.runtime_escrow_captures.remove(id);
            }
        }
    }
}

/// Validate each native row write without manufacturing consent from an existing row.
pub(crate) fn prepare_write(
    world: &WorldTransaction<'_, '_>,
    row: &AssetEscrowRecord,
    prepared: Option<PreparedCapture>,
) -> Result<PreparedWrite, Error> {
    if !enabled(world.runtime_selection.canonical()).map_err(refused)? {
        if prepared.is_some_and(|prepared| prepared.0.is_some()) {
            return Err(refused("prepared capture outlived its selected service"));
        }
        return Ok(PreparedWrite::Keep);
    }
    match world.asset_escrows.get(&row.id) {
        None => {
            let record = prepared
                .and_then(|prepared| prepared.0)
                .ok_or_else(|| refused("new custody has no prepared capture"))?;
            if !is_live(row.status)
                || record.opening != opening(row)?
                || !matches_kind(record.kind, row)
                || world.runtime_escrow_captures.get(&row.id).is_some()
            {
                return Err(refused("prepared capture differs from new custody"));
            }
            ensure_execution(world, &record.capture)?;
            Ok(PreparedWrite::Insert(row.id, record))
        }
        Some(previous) => {
            if prepared.is_some() || opening(previous)? != opening(row)? {
                return Err(refused("existing custody identity cannot be replaced"));
            }
            if !is_live(previous.status) {
                if is_live(row.status) || world.runtime_escrow_captures.get(&row.id).is_some() {
                    return Err(refused("terminal custody cannot regain a capture"));
                }
                return Ok(PreparedWrite::Keep);
            }
            let record = world
                .runtime_escrow_captures
                .get(&row.id)
                .ok_or_else(|| refused("live custody capture is absent"))?;
            if record.opening != opening(previous)? || !matches_kind(record.kind, previous) {
                return Err(refused("retained capture differs from prior custody"));
            }
            ensure_execution(world, &record.capture)?;
            Ok(if is_live(row.status) {
                PreparedWrite::Keep
            } else {
                PreparedWrite::Remove(row.id)
            })
        }
    }
}

/// Service activation/retirement never invents captures for custody that already exists.
pub(crate) fn check_transition(
    world: &impl WorldReadOnly,
    before: &RootSelectionV1,
    after: &RootSelectionV1,
) -> Result<(), String> {
    let before_enabled = enabled(before)?;
    let after_enabled = enabled(after)?;
    if before_enabled {
        validate_persistence(world).map_err(|error| error.to_string())?;
        if live_orderbook_count(world) != 0
            && let Some(reason) = orderbook_admission_gap(after)
        {
            return Err(reason);
        }
        let lookup = PublishedDefinitions(world.runtime_definitions());
        for (_, record) in world
            .runtime_escrow_captures()
            .iter()
            .filter(|(_, record)| record.kind == Kind::OrderbookLock)
        {
            record
                .capture
                .execution_selection(after, &lookup)
                .map_err(|error| error.to_string())?;
            if let Some(reason) = record
                .capture
                .broken_by(before, after, &lookup)
                .map_err(|error| error.to_string())?
            {
                return Err(format!(
                    "live orderbook custody retains its whole native batch graph: {reason}"
                ));
            }
        }
    }
    let catalog = super::release_catalog().map_err(|error| error.to_string())?;
    let storage = catalog
        .additions
        .iter()
        .find(|addition| addition.module == super::catalog::additions::NATIVE_ESCROW_CAPTURE.module)
        .and_then(|addition| addition.storage.get("world.runtime_escrow_captures"));
    if let Some(storage) = storage {
        if after_enabled != after.installed_storage.binary_search(storage).is_ok() {
            return Err("native escrow service and custody table must be selected together".into());
        }
    } else if before_enabled || after_enabled {
        return Err("native escrow service has no compiled custody table".into());
    }
    if before_enabled != after_enabled
        && (world
            .asset_escrows()
            .iter()
            .any(|(_, row)| is_live(row.status))
            || world.runtime_escrow_captures().iter().next().is_some())
    {
        return Err(
            "native escrow service transition requires every existing custody obligation to drain"
                .into(),
        );
    }
    Ok(())
}

/// Parent operations that can drain native orderbook custody still use ordinary admission.
/// Their whole binding graph is pinned; Submit and Match remain new work and may be closed.
// TODO: Replace this conservative gate with authenticated bounded parent planners and
// captured completion dispatch that verifies every touched custody context agrees.
pub(crate) fn orderbook_admission_gap(selection: &RootSelectionV1) -> Option<String> {
    use iroha_data_model::{isi::sorafs::*, runtime::definition::RestrictionV1};
    for wire in [
        wire_id_of::<CancelSorafsOrderbookOrder>(),
        wire_id_of::<MaintainSorafsOrderbook>(),
        wire_id_of::<RecordSorafsOrderbookSettlementReceipt>(),
    ] {
        let wire = wire.expect("native orderbook completion is registered");
        let Some(binding) = selection
            .bindings
            .iter()
            .find(|binding| binding.wire_id == wire)
        else {
            return Some(format!(
                "live orderbook custody requires its native terminal {wire}"
            ));
        };
        for definition in [binding.interface, binding.implementation] {
            if selection.restrictions.iter().any(|entry| {
                entry.definition == definition
                    && matches!(
                        entry.restriction,
                        RestrictionV1::AdmissionClosed(_) | RestrictionV1::CompletionOnly(_)
                    )
            }) {
                return Some(format!(
                    "live orderbook custody requires ordinary admission for {wire} ({definition})"
                ));
            }
        }
    }
    None
}

/// Count selected live orderbook custody, which has no generic escrow completion corridor.
pub(crate) fn live_orderbook_count(world: &impl WorldReadOnly) -> usize {
    if enabled(world.runtime_selection()).ok() != Some(true) {
        return 0;
    }
    world
        .runtime_escrow_captures()
        .iter()
        .filter(|(id, record)| {
            record.kind == Kind::OrderbookLock
                && world
                    .asset_escrows()
                    .get(id)
                    .is_some_and(|row| is_live(row.status))
        })
        .count()
}

/// Original captures of live native custody, grouped without granting new capabilities.
pub(crate) fn live_captures(world: &WorldTransaction<'_, '_>) -> Vec<(CompletionCaptureV1, usize)> {
    if enabled(world.runtime_selection.canonical()).ok() != Some(true) {
        return Vec::new();
    }
    let mut captures = std::collections::BTreeMap::new();
    for (id, record) in world.runtime_escrow_captures.iter() {
        if world
            .asset_escrows
            .get(id)
            .is_some_and(|row| is_live(row.status))
        {
            *captures.entry(record.capture.clone()).or_default() += 1;
        }
    }
    captures.into_iter().collect()
}

/// Whether a live row still needs conservative predecessor dispatch, including batched orderbook ownership.
pub(crate) fn pins_predecessor(world: &WorldTransaction<'_, '_>, id: &EscrowId) -> bool {
    enabled(world.runtime_selection.canonical()).ok() != Some(true)
        || world
            .runtime_escrow_captures
            .get(id)
            .is_none_or(|record| record.kind == Kind::OrderbookLock)
}

/// A successor retains every original native custody graph even after ordinary bindings move.
pub(crate) fn closure_gap(
    predecessor: &RootSelectionV1,
    successor: &RootSelectionV1,
    published: &impl iroha_data_model::runtime::definition::DefinitionLookup,
    captures: &[(CompletionCaptureV1, usize)],
) -> Option<String> {
    captures.iter().find_map(|(capture, count)| {
        let context = match capture.execution_selection(successor, published) {
            Ok(context) => context,
            Err(error) => return Some(format!("{count} live native escrow obligations: {error}")),
        };
        let reason = match capture.broken_by(predecessor, &context, published) {
            Ok(reason) => reason,
            Err(error) => Some(error.to_string()),
        }?;
        Some(format!("{count} live native escrow obligations: {reason}"))
    })
}

/// Restore refuses a selected table with missing, orphaned or mismatched authoritative captures.
pub(crate) fn validate_persistence(world: &impl WorldReadOnly) -> Result<(), Error> {
    if !enabled(world.runtime_selection()).map_err(refused)? {
        return Ok(());
    }
    if live_orderbook_count(world) != 0
        && let Some(reason) = orderbook_admission_gap(world.runtime_selection())
    {
        return Err(refused(reason));
    }
    for (id, row) in world
        .asset_escrows()
        .iter()
        .filter(|(_, row)| is_live(row.status))
    {
        let record = world
            .runtime_escrow_captures()
            .get(id)
            .ok_or_else(|| refused("restored live custody lacks its capture"))?;
        if !matches_kind(record.kind, row) || record.opening != opening(row)? {
            return Err(refused("restored custody capture differs from its row"));
        }
        let context = record
            .capture
            .execution_selection(
                world.runtime_selection(),
                &PublishedDefinitions(world.runtime_definitions()),
            )
            .map_err(refused)?;
        if record.kind == Kind::OrderbookLock
            && let Some(reason) = record
                .capture
                .broken_by(
                    &context,
                    world.runtime_selection(),
                    &PublishedDefinitions(world.runtime_definitions()),
                )
                .map_err(refused)?
        {
            return Err(refused(reason));
        }
    }
    for (id, _) in world.runtime_escrow_captures().iter() {
        if !world
            .asset_escrows()
            .get(id)
            .is_some_and(|row| is_live(row.status))
        {
            return Err(refused("restored custody capture has no live row"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
