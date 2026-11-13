//! Discovery and certified historical proofs for the selected game transfer protocol.

use super::*;
use iroha_data_model::runtime::native_game::*;

/// Read the original local transfer record for discovery; this is not foreign proof authority.
///
/// # Errors
/// Invalid retained custody or a key belonging to another world is rejected.
pub fn native_game_transfer_snapshot(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<NativeGameTransferV1>, String> {
    let Some(record) = view.world().runtime_game_transfers().get(&transfer_id) else {
        return Ok(None);
    };
    record.validate()?;
    if record.intent.world_id != world_id || record.intent.transfer_id() != transfer_id {
        return Err("native game transfer discovery identity mismatch".into());
    }
    Ok(Some(record.clone()))
}

/// Read the certified source intent and original unit lock.
/// Current State supplies discovery only; exact original archived bytes and finality authorize it.
///
/// # Errors
/// Invalid finality, archive substitution, malformed custody, or wrong discovery identity.
pub fn game_unit_export_intent_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<GameUnitExportIntentProofV1>, String> {
    let Some(row) = native_game_transfer_snapshot(view, world_id, transfer_id)? else {
        return Ok(None);
    };
    if row.role != NativeGameRoleV1::Source {
        return Ok(None);
    }
    let record = &row.intent;
    let value = norito::encode_canonical(record).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        record.source_height,
        record.witness_key(),
        &value,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(GameUnitExportIntentProofV1 {
        record: record.clone(),
        block,
        path,
    }))
}

/// Read the certified destination reservation retained for the original source request.
/// Current State supplies discovery only; exact original archived bytes and finality authorize it.
///
/// # Errors
/// Invalid finality, archive substitution, malformed custody, or wrong discovery identity.
pub fn game_unit_reservation_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<GameUnitReservationProofV1>, String> {
    let Some(row) = native_game_transfer_snapshot(view, world_id, transfer_id)? else {
        return Ok(None);
    };
    if row.role != NativeGameRoleV1::Destination {
        return Ok(None);
    }
    let Some(record) = row.reservation.as_ref() else {
        return Ok(None);
    };
    let value = norito::encode_canonical(record).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        record.destination_height,
        record.witness_key(),
        &value,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(GameUnitReservationProofV1 {
        record: record.clone(),
        block,
        path,
    }))
}

/// Read the certified irreversible source removal under its accepted destination reservation.
/// Current State supplies discovery only; exact original archived bytes and finality authorize it.
///
/// # Errors
/// Invalid finality, archive substitution, malformed custody, or wrong discovery identity.
pub fn reserved_game_unit_export_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<ReservedGameUnitExportProofV1>, String> {
    let Some(row) = native_game_transfer_snapshot(view, world_id, transfer_id)? else {
        return Ok(None);
    };
    if row.role != NativeGameRoleV1::Source {
        return Ok(None);
    }
    let Some(record) = row.export.as_ref() else {
        return Ok(None);
    };
    let value = norito::encode_canonical(record).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        record.export.source_height,
        record.witness_key(),
        &value,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(ReservedGameUnitExportProofV1 {
        record: record.clone(),
        block,
        path,
    }))
}

/// Read the authentic destination import acknowledgement needed for final source closure.
/// Current State supplies discovery only; exact original archived bytes and finality authorize it.
///
/// # Errors
/// Invalid finality, archive substitution, malformed custody, or wrong discovery identity.
pub fn game_unit_import_ack_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<GameUnitImportAckProofV1>, String> {
    let Some(row) = native_game_transfer_snapshot(view, world_id, transfer_id)? else {
        return Ok(None);
    };
    if row.role != NativeGameRoleV1::Destination {
        return Ok(None);
    }
    let Some(record) = row.acknowledgement.as_ref() else {
        return Ok(None);
    };
    let value = norito::encode_canonical(record).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        record.destination_height,
        record.witness_key(),
        &value,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(GameUnitImportAckProofV1 {
        record: record.clone(),
        block,
        path,
    }))
}

/// Read the certified permanent source cancellation of an unexported original request.
/// Current State supplies discovery only; exact original archived bytes and finality authorize it.
///
/// # Errors
/// Invalid finality, archive substitution, malformed custody, or wrong discovery identity.
pub fn game_unit_export_cancel_proof(
    view: &impl StateReadOnly,
    world_id: Hash,
    transfer_id: Hash,
) -> Result<Option<GameUnitExportCancelProofV1>, String> {
    let Some(row) = native_game_transfer_snapshot(view, world_id, transfer_id)? else {
        return Ok(None);
    };
    if row.role != NativeGameRoleV1::Source {
        return Ok(None);
    }
    let Some(record) = row.cancellation.as_ref() else {
        return Ok(None);
    };
    let value = norito::encode_canonical(record).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        record.source_height,
        record.witness_key(),
        &value,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(GameUnitExportCancelProofV1 {
        record: record.clone(),
        block,
        path,
    }))
}
