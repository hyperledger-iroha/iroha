//! Selected native transfer handlers: every custody change follows its original certified write.

use super::*;
use crate::runtime::native_game_capture as custody;
use iroha_data_model::{
    runtime::native_game::*,
    sumeragi_amx::{AmxCertifiedBlockV1, AmxWriteProofV1},
};

/// One canonical record frame: a published witness value or the value a proof path commits.
///
/// The fixed flags, schema, header, padding and checksum equal `norito::encode_canonical`, so
/// archived proof readers reproduce these exact bytes. Unlike that growable sink, the counted
/// frame is admitted against the caller's enclosing decode-allocation budget and reserved
/// fallibly before the second pass writes it. A local refusal therefore keeps its original owner
/// instead of aborting or becoming a deterministic rejection.
/// TODO: retain the published witness bytes in the transaction's finite pool.
fn encoded<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>, AmxError> {
    use norito::core::BoundedEncodeError;
    norito::core::classify_decode_attempt(|| {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        match norito::core::to_bytes_bounded(value, GAME_PARTITION_MAX_PROOF_BYTES) {
            Ok(bytes) => Ok(Ok(bytes)),
            // A record is one member of its proof graph and cannot exceed that graph's bound.
            Err(BoundedEncodeError::FrameTooLarge { .. }) => Ok(Err(AmxError::State(
                "native game record exceeds its finite frame bound",
            ))),
            Err(BoundedEncodeError::AllocationFailed { bytes }) => {
                Err(norito::Error::AllocationFailed {
                    bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
                })
            }
            Err(BoundedEncodeError::Serialization(error)) => Err(error),
        }
    })
    .map_err(proof_encoding_error)?
}
fn admission(error: AmxError, state: &mut StateTransaction<'_, '_>) -> Error {
    crate::sumeragi::amx::amx_error(&error, state)
}
fn source_height(state: &StateTransaction<'_, '_>) -> Result<u64, Error> {
    let height = state.block_height();
    if height < 2 {
        return Err(invalid(
            "game custody requires a certified non-genesis carrier",
        ));
    }
    Ok(height)
}
fn record(state: &StateTransaction<'_, '_>, id: Hash) -> Result<NativeGameTransferV1, Error> {
    state
        .world
        .runtime_game_transfers
        .get(&id)
        .cloned()
        .ok_or_else(|| invalid("unknown native game transfer"))
}
fn room(state: &StateTransaction<'_, '_>, world: Hash) -> Result<(), Error> {
    if state
        .world
        .runtime_game_transfers
        .iter()
        .filter(|(_, row)| row.intent.world_id == world)
        .count()
        >= GAME_PARTITION_MAX_RECEIPTS
    {
        return Err(invalid("game transfer identity retention is full"));
    }
    Ok(())
}
fn validate_record(row: &NativeGameTransferV1) -> Result<(), Error> {
    row.validate_shape().map_err(invalid)
}
fn validate_partition(
    state: &StateTransaction<'_, '_>,
    partition: &GamePartitionStateV1,
    completing: Option<Hash>,
) -> Result<(), Error> {
    partition.validate().map_err(invalid)?;
    custody::check_capacity_after(state.world(), partition, completing)
}
/// Preserve the original codec refusal before any witness or State mutation.
fn witness<T: norito::NoritoSerialize>(
    state: &mut StateTransaction<'_, '_>,
    key: Vec<u8>,
    value: &T,
) -> Result<(), Error> {
    let value = encoded(value).map_err(|error| admission(error, state))?;
    crate::exec_witness::record_write_native_game(key, value);
    Ok(())
}
/// Exact registered peer finality and ordinary-write inclusion; endpoints supply no authority.
fn verify<T: norito::NoritoSerialize, P: norito::NoritoSerialize>(
    partition: &GamePartitionStateV1,
    peer: [u8; 32],
    height: u64,
    key: Vec<u8>,
    value: &T,
    block: &AmxCertifiedBlockV1,
    path: &AmxWriteProofV1,
    whole: &P,
) -> Result<(), AmxError> {
    if height < 2
        || peer == partition.local_instance
        || block.consensus_header.len() > 8192
        || block.commit_qc.len() > 128 * 1024
        || block.result_preimage.len() > GAME_PARTITION_MAX_PROOF_BYTES - 140 * 1024
        || path.siblings.len() > 256
    {
        return Err(AmxError::State(
            "native game proof exceeds its carrier bounds",
        ));
    }
    if norito::core::classify_decode_attempt(|| norito::canonical_frame_len(whole))
        .map_err(proof_encoding_error)?
        > GAME_PARTITION_MAX_PROOF_BYTES
    {
        return Err(AmxError::State(
            "native game proof graph exceeds finite bound",
        ));
    }
    let peer = partition
        .peers
        .iter()
        .find(|candidate| candidate.tracker.instance == peer)
        .ok_or(AmxError::State(
            "native game proof names an unregistered peer",
        ))?;
    let verified = peer.tracker.verify_block(block)?;
    if verified.height != height {
        return Err(AmxError::State(
            "native game record differs from certified carrier height",
        ));
    }
    let bytes = encoded(value)?;
    if path.root(&key, &bytes)? != verified.commitment().execution.ordinary_writes_root {
        return Err(AmxError::State(
            "native game record is absent from certified ordinary writes",
        ));
    }
    Ok(())
}
fn require_role(row: &NativeGameTransferV1, role: NativeGameRoleV1) -> Result<(), Error> {
    validate_record(row)?;
    if row.role != role {
        return Err(invalid(
            "native game operation addresses the other root's custody",
        ));
    }
    Ok(())
}
fn digest(export: &ReservedGameUnitExportV1) -> Result<Hash, AmxError> {
    encoded(export).map(|bytes| Hash::new_from_chunks(&[b"iroha/game-reserved-export/v1", &bytes]))
}

impl Execute for RequestGameUnitExportV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let capture = custody::prepare(state, NativeGameRoleV1::Source)?;
        let partition = read(state, &self.world_id)?;
        custody::ensure_unlocked(state.world(), self.world_id, self.unit_id)?;
        let at = owned_index(&partition, self.unit_id, authority)?;
        if self.destination == partition.local_instance
            || !partition
                .peers
                .iter()
                .any(|peer| peer.tracker.instance == self.destination)
        {
            return Err(invalid(
                "native game request has no registered foreign destination",
            ));
        }
        let mut unit = partition.units[at].clone();
        unit.sequence = unit
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("game transfer sequence overflow"))?;
        let intent = GameUnitExportIntentV1 {
            world_id: self.world_id,
            source: partition.local_instance,
            destination: self.destination,
            kernel_hash: partition.kernel_hash,
            unit,
            nonce: self.nonce,
            source_height: source_height(state)?,
        };
        intent.validate().map_err(invalid)?;
        let id = intent.transfer_id();
        if state.world.runtime_game_transfers.get(&id).is_some() {
            return Err(invalid("native game request identity was already used"));
        }
        room(state, self.world_id)?;
        let requests = state
            .world
            .runtime_game_transfers
            .iter()
            .filter(|(_, row)| {
                row.role == NativeGameRoleV1::Source
                    && row.capture.is_some()
                    && row.export.is_none()
                    && row.intent.world_id == self.world_id
            })
            .count();
        if partition.exports.len() + requests >= GAME_PARTITION_MAX_RECEIPTS {
            return Err(invalid("native game source export capacity is reserved"));
        }
        let row = NativeGameTransferV1 {
            role: NativeGameRoleV1::Source,
            intent,
            reservation: None,
            export: None,
            acknowledgement: None,
            cancellation: None,
            capture: Some(capture),
        };
        validate_record(&row)?;
        witness(state, row.intent.witness_key(), &row.intent)?;
        state.world.runtime_game_transfers.insert(id, row);
        Ok(())
    }
}
impl Execute for ReserveGameUnitImportV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let capture = custody::prepare(state, NativeGameRoleV1::Destination)?;
        let intent = &self.proof.record;
        intent.validate().map_err(invalid)?;
        let partition = read(state, &intent.world_id)?;
        // The destination executing here is this partition's own signed-root instance.
        if (intent.destination, intent.kernel_hash)
            != (partition.local_instance, partition.kernel_hash)
        {
            return Err(invalid(
                "native game intent names another destination or kernel",
            ));
        }
        verify(
            &partition,
            intent.source,
            intent.source_height,
            intent.witness_key(),
            intent,
            &self.proof.block,
            &self.proof.path,
            &self.proof,
        )
        .map_err(|error| admission(error, state))?;
        let id = intent.transfer_id();
        if let Some(prior) = state.world.runtime_game_transfers.get(&id) {
            return if prior.role == NativeGameRoleV1::Destination && prior.intent == *intent {
                Ok(())
            } else {
                Err(invalid("native game reservation identity collision"))
            };
        }
        room(state, intent.world_id)?;
        if partition
            .units
            .iter()
            .any(|unit| unit.unit_id == intent.unit.unit_id)
            || partition.known_units.iter().any(|known| {
                known.unit_id == intent.unit.unit_id && known.sequence >= intent.unit.sequence
            })
            || state.world.runtime_game_transfers.iter().any(|(_, row)| {
                row.role == NativeGameRoleV1::Destination
                    && row.capture.is_some()
                    && row.intent.world_id == intent.world_id
                    && row.intent.unit.unit_id == intent.unit.unit_id
            })
        {
            return Err(invalid(
                "native game reservation conflicts with unit custody",
            ));
        }
        let pending: Vec<_> = state
            .world
            .runtime_game_transfers
            .iter()
            .filter(|(_, row)| {
                row.role == NativeGameRoleV1::Destination
                    && row.capture.is_some()
                    && row.intent.world_id == intent.world_id
            })
            .map(|(_, row)| &row.intent.unit)
            .collect();
        let unknown = pending
            .iter()
            .filter(|unit| {
                !partition
                    .known_units
                    .iter()
                    .any(|known| known.unit_id == unit.unit_id)
            })
            .count();
        let new_identity = usize::from(
            !partition
                .known_units
                .iter()
                .any(|known| known.unit_id == intent.unit.unit_id),
        );
        if partition.units.len() + pending.len() >= GAME_PARTITION_MAX_UNITS
            || partition.imported.len() + pending.len() >= GAME_PARTITION_MAX_RECEIPTS
            || partition.known_units.len() + unknown + new_identity > GAME_PARTITION_MAX_RECEIPTS
        {
            return Err(invalid(
                "native game destination reservation capacity is full",
            ));
        }
        let reservation = GameUnitReservationV1 {
            intent: intent.clone(),
            destination_height: source_height(state)?,
        };
        let row = NativeGameTransferV1 {
            role: NativeGameRoleV1::Destination,
            intent: intent.clone(),
            reservation: Some(reservation),
            export: None,
            acknowledgement: None,
            cancellation: None,
            capture: Some(capture),
        };
        validate_record(&row)?;
        let reservation = row
            .reservation
            .as_ref()
            .expect("prepared destination reservation");
        witness(state, reservation.witness_key(), reservation)?;
        state.world.runtime_game_transfers.insert(id, row);
        Ok(())
    }
}
impl Execute for ExportReservedGameUnitV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let reservation = &self.proof.record;
        reservation.validate().map_err(invalid)?;
        let id = reservation.transfer_id();
        let mut row = record(state, id)?;
        require_role(&row, NativeGameRoleV1::Source)?;
        if reservation.intent != row.intent || row.cancellation.is_some() {
            return Err(invalid(
                "native game reservation does not fulfill the live source request",
            ));
        }
        let partition = read(state, &row.intent.world_id)?;
        verify(
            &partition,
            row.intent.destination,
            reservation.destination_height,
            reservation.witness_key(),
            reservation,
            &self.proof.block,
            &self.proof.path,
            &self.proof,
        )
        .map_err(|error| admission(error, state))?;
        if row.export.is_some() {
            return if row.reservation.as_ref() == Some(reservation) {
                Ok(())
            } else {
                Err(invalid(
                    "native game source accepted another original reservation",
                ))
            };
        }
        custody::guard(state, &row)?;
        let (next, export) = export_candidate(
            partition,
            row.intent.unit.unit_id,
            row.intent.destination,
            &row.intent.unit.owner,
            source_height(state)?,
        )?;
        if export.unit != row.intent.unit
            || export.source != row.intent.source
            || export.kernel_hash != row.intent.kernel_hash
        {
            return Err(invalid(
                "native game locked source unit changed before export",
            ));
        }
        let exported = ReservedGameUnitExportV1 {
            reservation: reservation.clone(),
            export,
        };
        exported.validate().map_err(invalid)?;
        validate_partition(state, &next, Some(id))?;
        row.reservation = Some(reservation.clone());
        row.export = Some(exported);
        validate_record(&row)?;
        let exported = row.export.as_ref().expect("prepared source export");
        witness(state, exported.witness_key(), exported)?;
        state.world.game_partitions.insert(next.world_id, next);
        state.world.runtime_game_transfers.insert(id, row);
        Ok(())
    }
}
impl Execute for ImportReservedGameUnitV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let exported = &self.proof.record;
        exported.validate().map_err(invalid)?;
        let id = exported.reservation.transfer_id();
        let mut row = record(state, id)?;
        require_role(&row, NativeGameRoleV1::Destination)?;
        if row.reservation.as_ref() != Some(&exported.reservation) || row.cancellation.is_some() {
            return Err(invalid(
                "native game export has no exact live destination reservation",
            ));
        }
        let partition = read(state, &row.intent.world_id)?;
        verify(
            &partition,
            row.intent.source,
            exported.export.source_height,
            exported.witness_key(),
            exported,
            &self.proof.block,
            &self.proof.path,
            &self.proof,
        )
        .map_err(|error| admission(error, state))?;
        if row.acknowledgement.is_some() {
            return if row.export.as_ref() == Some(exported) {
                Ok(())
            } else {
                Err(invalid("native game destination imported another export"))
            };
        }
        custody::guard(state, &row)?;
        if partition.imported.contains(&exported.export.receipt_id()) {
            return Err(invalid(
                "native game reserved import lacks its original ACK",
            ));
        }
        let next = insert_import(partition, &exported.export)?;
        validate_partition(state, &next, Some(id))?;
        let ack = GameUnitImportAckV1 {
            transfer_id: id,
            export_hash: digest(exported).map_err(|error| admission(error, state))?,
            destination_height: source_height(state)?,
        };
        row.export = Some(exported.clone());
        row.acknowledgement = Some(ack);
        row.capture = None;
        validate_record(&row)?;
        witness(state, ack.witness_key(), &ack)?;
        state.world.game_partitions.insert(next.world_id, next);
        state.world.runtime_game_transfers.insert(id, row);
        Ok(())
    }
}
impl Execute for AcknowledgeGameUnitImportV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let ack = self.proof.record;
        let mut row = record(state, ack.transfer_id)?;
        require_role(&row, NativeGameRoleV1::Source)?;
        let exported = row
            .export
            .as_ref()
            .ok_or_else(|| invalid("native game ACK precedes source export"))?;
        if ack.destination_height == 0
            || ack.export_hash != digest(exported).map_err(|error| admission(error, state))?
        {
            return Err(invalid(
                "native game ACK differs from original exported custody",
            ));
        }
        let partition = read(state, &row.intent.world_id)?;
        verify(
            &partition,
            row.intent.destination,
            ack.destination_height,
            ack.witness_key(),
            &ack,
            &self.proof.block,
            &self.proof.path,
            &self.proof,
        )
        .map_err(|error| admission(error, state))?;
        if let Some(prior) = row.acknowledgement {
            return if prior == ack {
                Ok(())
            } else {
                Err(invalid("native game ACK identity changed"))
            };
        }
        custody::guard(state, &row)?;
        row.acknowledgement = Some(ack);
        row.capture = None;
        validate_record(&row)?;
        state
            .world
            .runtime_game_transfers
            .insert(ack.transfer_id, row);
        Ok(())
    }
}
impl Execute for CancelGameUnitExportRequestV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let mut row = record(state, self.transfer_id)?;
        require_role(&row, NativeGameRoleV1::Source)?;
        if row.intent.unit.owner != *authority || row.export.is_some() {
            return Err(invalid(
                "only the original owner may cancel an unexported native game request",
            ));
        }
        if row.cancellation.is_some() {
            return Ok(());
        }
        custody::guard(state, &row)?;
        let partition = read(state, &row.intent.world_id)?;
        let at = owned_index(&partition, row.intent.unit.unit_id, authority)?;
        let mut expected = row.intent.unit.clone();
        expected.sequence -= 1;
        if partition.units[at] != expected {
            return Err(invalid(
                "native game cancellation lost its original locked unit",
            ));
        }
        let cancellation = GameUnitExportCancelV1 {
            intent: row.intent.clone(),
            source_height: source_height(state)?,
        };
        row.cancellation = Some(cancellation);
        row.capture = None;
        validate_record(&row)?;
        let cancellation = row
            .cancellation
            .as_ref()
            .expect("prepared original cancellation");
        witness(state, cancellation.witness_key(), cancellation)?;
        state
            .world
            .runtime_game_transfers
            .insert(self.transfer_id, row);
        Ok(())
    }
}
impl Execute for CancelReservedGameUnitImportV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let cancellation = &self.proof.record;
        cancellation.intent.validate().map_err(invalid)?;
        let id = cancellation.intent.transfer_id();
        let mut row = record(state, id)?;
        require_role(&row, NativeGameRoleV1::Destination)?;
        if row.intent != cancellation.intent
            || row.export.is_some()
            || cancellation.source_height == 0
        {
            return Err(invalid(
                "native game cancellation differs from an unimported reservation",
            ));
        }
        let partition = read(state, &row.intent.world_id)?;
        verify(
            &partition,
            row.intent.source,
            cancellation.source_height,
            cancellation.witness_key(),
            cancellation,
            &self.proof.block,
            &self.proof.path,
            &self.proof,
        )
        .map_err(|error| admission(error, state))?;
        if let Some(prior) = &row.cancellation {
            return if prior == cancellation {
                Ok(())
            } else {
                Err(invalid("native game cancellation identity changed"))
            };
        }
        custody::guard(state, &row)?;
        row.cancellation = Some(cancellation.clone());
        row.capture = None;
        validate_record(&row)?;
        state.world.runtime_game_transfers.insert(id, row);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
