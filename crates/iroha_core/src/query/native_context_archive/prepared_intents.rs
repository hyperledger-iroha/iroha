//! Local, unresolved outbound Prepared intent from the original executed witness.
//!
//! One canonical frame borrows exact coordinates and a checked scalar row sequence. These are source coordinates, never finality or signing authority. No
//! AMX record graph is decoded. The unchanged `.nrt` is the complete original record
//! source, which an eventual consumer must authenticate with the native carrier.
//! TODO(S6): consume with the owned proof reader and an explicitly authorized, funded
//! parent transaction owner. No submit or successful relay is represented here.

use super::*;
use crate::state::CapturedExecWitness;
use iroha_crypto::Hash;
use iroha_data_model::{
    NetworkId,
    execution_witness::ExecutionWitnessKeyTagV1,
    sumeragi_amx::{AMX_RECORD_WITNESS_KEY_BYTES, AmxRecordKind},
};
use iroha_model_base::topology::DataSpaceId;

#[derive(norito::Encode, norito::NoritoSchema)]
// The borrowed root explicitly projects to the sole owned frame identity; its nominal
// erased lifetime stays distinct, as required by Norito's schema contract.
#[norito_schema(
    name = "iroha_core::amx::PreparedIntentCoordinatesV1",
    frame = "iroha_core::amx::PreparedIntentCoordinatesV1"
)]
struct Coordinates<'a> {
    local_network: NetworkId,
    local_chain: &'a [u8],
    parent_network: NetworkId,
    parent_chain: &'a [u8],
    parent_instance: [u8; 32],
    parent_genesis: Hash,
    parent_successor: Hash,
    participant: DataSpaceId,
    height: u64,
    carrier: HashOf<BlockHeader>,
    ordinary_writes_root: Hash,
    records: u64,
    // Zero always means outbound signing/fee authority is unresolved.
    authority: u8,
    rows: RowsRef<'a>,
}
#[derive(norito::Encode, norito::NoritoSchema)]
#[cfg_attr(test, derive(norito::Decode))]
#[norito_schema(name = "iroha_core::amx::PreparedIntentRowV1")]
struct Row {
    key: [u8; AMX_RECORD_WITNESS_KEY_BYTES],
    tx: [u8; 32],
    participant: DataSpaceId,
    value: Hash,
}

pub(super) fn is_prepared_key_prefix(key: &[u8]) -> bool {
    key.first() == Some(&(ExecutionWitnessKeyTagV1::AmxRecord as u8))
        && key.get(1) == Some(&(AmxRecordKind::Prepared as u8))
}
fn prepared_key(
    key: &[u8],
) -> Result<Option<[u8; AMX_RECORD_WITNESS_KEY_BYTES]>, NativeContextArchiveError> {
    if !is_prepared_key_prefix(key) {
        return Ok(None);
    }
    Ok(Some(key.try_into().map_err(|_| {
        NativeContextArchiveError::Source("original Prepared key length differs")
    })?))
}
fn row(write: &ExecKv, participant: DataSpaceId) -> Result<Option<Row>, NativeContextArchiveError> {
    let Some(key) = prepared_key(&write.key)? else {
        return Ok(None);
    };
    let mut tx = [0; 32];
    tx.copy_from_slice(&key[2..]);
    Ok(Some(Row {
        key,
        tx,
        participant,
        value: Hash::new(&write.value),
    }))
}
fn count(writes: &[ExecKv]) -> Result<usize, NativeContextArchiveError> {
    let mut count = 0usize;
    for write in writes {
        if prepared_key(&write.key)?.is_some() {
            count = count
                .checked_add(1)
                .ok_or(iroha_allocation::AllocationRefusal::DemandOverflow)
                .map_err(ChargedBufferError::Admission)?;
        }
    }
    Ok(count)
}

// Reuse Norito's actual canonical element-sequence writer, never a copied codec or Vec.
struct RowsRef<'a> {
    writes: &'a [ExecKv],
    participant: DataSpaceId,
    count: usize,
}
impl norito::NoritoSchema for RowsRef<'_> {
    fn nominal_name() -> String {
        <Vec<Row> as norito::NoritoSchema>::nominal_name()
    }
    fn frame_name() -> String {
        <Vec<Row> as norito::NoritoSchema>::frame_name()
    }
}
impl norito::core::SerializePayload for RowsRef<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        norito::core::write_element_sequence::<Row, _>(
            writer,
            Rows {
                writes: self.writes.iter(),
                participant: self.participant,
                remaining: self.count,
            },
        )
    }
}
struct Rows<'a> {
    writes: std::slice::Iter<'a, ExecKv>,
    participant: DataSpaceId,
    remaining: usize,
}
impl Iterator for Rows<'_> {
    type Item = Row;
    fn next(&mut self) -> Option<Row> {
        for write in self.writes.by_ref() {
            if is_prepared_key_prefix(&write.key) {
                // count() checked every exact key before constructing this immutable view.
                // A cardinality/length fault is refused by the shared sequence writer.
                let item = row(write, self.participant).ok().flatten()?;
                self.remaining = self.remaining.checked_sub(1)?;
                return Some(item);
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for Rows<'_> {}

impl NativeContextArchive {
    /// Complete the same context's local Prepared intent before State publication.
    /// Refusal leaves the completed canonical context and original execution live.
    pub(crate) fn prepare_amx_intents(
        &self,
        original: &mut PreparedNativeContext,
        overlay: &StateBlock<'_>,
        executed: &SignedBlock,
        result: &RetainedPayload<ExecutionResultCommitment>,
        witness: &CapturedExecWitness,
    ) -> Result<(), NativeContextArchiveError> {
        // Ordinary no-AMX blocks retain their original capture path unchanged.
        if original.intents_complete && original.prepared_intents.is_none() {
            return Ok(());
        }
        self.recheck_namespace()?;
        if !self.writable
            || !original.bytes.belongs_to(&self.budget)
            || !result.belongs_to(&self.budget)
            || !witness.pool().same_pool(&self.budget)
            || !witness.matches_original_native_execution(executed, result.get().execution)
            || original.height != executed.header().height().get()
            || original.carrier_hash != executed.hash()
            || executed.header() != overlay._curr_block
            || result.get().schedule.current.network_id != *overlay.network_id()
        {
            return Err(NativeContextArchiveError::Source(
                "intent lost its original execution or pool",
            ));
        }
        if original.intents_complete {
            return Ok(());
        }
        let records = count(&witness.writes)?;
        if records == 0 {
            original.intents_complete = true;
            return Ok(());
        }
        let parent = overlay
            .world()
            .sumeragi_amx_participant()
            .authenticated_parent_source(&self.budget)
            .ok_or(NativeContextArchiveError::Source(
                "Prepared intent lacks its original authenticated participant",
            ))?;
        let global = &parent.participant.global;
        // Never consult participant.prepared: same-block settle/prune may remove an
        // entry while the original transactionally committed witness retains it.
        let coordinates = Coordinates {
            local_network: *overlay.network_id(),
            local_chain: overlay.chain_id().as_str().as_bytes(),
            parent_network: global.current.network_id,
            parent_chain: &parent.global_chain_label,
            parent_instance: global.instance,
            parent_genesis: Hash::new(&parent.global_genesis),
            parent_successor: Hash::new(&parent.global_successor),
            participant: parent.participant.dataspace,
            height: original.height,
            carrier: original.carrier_hash,
            ordinary_writes_root: result.get().execution.ordinary_writes_root,
            records: u64::try_from(records).map_err(|_| norito::Error::LengthMismatch)?,
            authority: 0,
            rows: RowsRef {
                writes: &witness.writes,
                participant: parent.participant.dataspace,
                count: records,
            },
        };
        let length = norito::canonical_frame_len(&coordinates)?;
        if length > self.maximum.get() {
            return Err(NativeContextArchiveError::Limit {
                maximum: self.maximum.get(),
                actual: length,
            });
        }
        // Admit every destination byte before encoding, from the original finite pool.
        let mut bytes = ChargedBuffer::new(length, &self.budget)?;
        norito::core::write_canonical_to_writer(&coordinates, &mut BufferWriter(&mut bytes))?;
        if bytes.as_slice().len() != length {
            return Err(norito::Error::LengthMismatch.into());
        }
        original.prepared_intents = Some(bytes);
        original.intents_complete = true;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    #[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_core::amx::PreparedIntentCoordinatesV1")]
    struct Decoded {
        local_network: NetworkId,
        local_chain: Vec<u8>,
        parent_network: NetworkId,
        parent_chain: Vec<u8>,
        parent_instance: [u8; 32],
        parent_genesis: Hash,
        parent_successor: Hash,
        participant: DataSpaceId,
        height: u64,
        carrier: HashOf<BlockHeader>,
        ordinary_writes_root: Hash,
        records: u64,
        authority: u8,
        rows: Vec<Row>,
    }
    pub(crate) fn verify_original(
        bytes: &[u8],
        overlay: &StateBlock<'_>,
        executed: &SignedBlock,
        result: &ExecutionResultCommitment,
        witness: &CapturedExecWitness,
    ) -> [u8; 32] {
        // Owning decode is a test oracle only; no durable decoder is added to production.
        let value: Decoded = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .unwrap();
        assert_eq!(norito::encode_canonical(&value).unwrap(), bytes);
        let parent = overlay
            .world()
            .sumeragi_amx_participant()
            .canonical()
            .unwrap();
        assert_eq!(value.local_network, *overlay.network_id());
        assert_eq!(value.local_chain, overlay.chain_id().as_str().as_bytes());
        assert_eq!(
            value.parent_network,
            parent.participant.global.current.network_id
        );
        assert_eq!(value.parent_chain, parent.global_chain_label);
        assert_eq!(value.parent_instance, parent.participant.global.instance);
        assert_eq!(value.parent_genesis, Hash::new(&parent.global_genesis));
        assert_eq!(value.parent_successor, Hash::new(&parent.global_successor));
        assert_eq!(value.participant, parent.participant.dataspace);
        assert_eq!(value.height, executed.header().height().get());
        assert_eq!(value.carrier, executed.hash());
        assert_eq!(
            value.ordinary_writes_root,
            result.execution.ordinary_writes_root
        );
        assert_eq!(
            value.authority, 0,
            "captured intent never resolves missing outbound authority"
        );
        assert_eq!(value.records, u64::try_from(value.rows.len()).unwrap());
        let expected: Vec<_> = witness
            .writes
            .iter()
            .filter_map(|write| row(write, value.participant).unwrap())
            .collect();
        assert_eq!(value.rows.len(), expected.len());
        for (actual, expected) in value.rows.iter().zip(&expected) {
            assert_eq!(actual.key, expected.key);
            assert_eq!(actual.tx, expected.tx);
            assert_eq!(actual.participant, expected.participant);
            assert_eq!(actual.value, expected.value);
        }
        assert!(
            !value.rows.is_empty(),
            "actual Prepared witness must capture an intent"
        );
        value.rows[0].tx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::sumeragi_amx::amx_record_witness_key;
    #[test]
    fn borrowed_intent_coordinates_declare_exact_owned_frame_without_erasing_nominal_lifetime() {
        use norito::NoritoSchema as _;
        assert_eq!(
            Coordinates::static_frame_name(),
            Some("iroha_core::amx::PreparedIntentCoordinatesV1")
        );
        assert_eq!(
            Coordinates::frame_name(),
            "iroha_core::amx::PreparedIntentCoordinatesV1"
        );
        assert_ne!(Coordinates::nominal_name(), Coordinates::frame_name());
        assert_eq!(
            norito::schema::identity::frame_hash::<Coordinates>(),
            norito::core::schema_hash_for_name("iroha_core::amx::PreparedIntentCoordinatesV1")
        );
    }
    #[test]
    fn original_prepared_key_selection_refuses_malformed_and_preserves_transaction() {
        let key = amx_record_witness_key(AmxRecordKind::Prepared, [0x41; 32]);
        assert!(is_prepared_key_prefix(&key));
        assert_eq!(prepared_key(&key).unwrap(), Some(key));
        assert!(matches!(
            prepared_key(&key[..33]),
            Err(NativeContextArchiveError::Source(_))
        ));
        assert!(
            prepared_key(&amx_record_witness_key(AmxRecordKind::Begin, [0x41; 32]))
                .unwrap()
                .is_none()
        );
        assert!(prepared_key(&[]).unwrap().is_none());
        let write = ExecKv {
            key: key.to_vec(),
            value: vec![1, 2, 3],
        };
        let selected = row(&write, DataSpaceId::new(5)).unwrap().unwrap();
        assert_eq!(selected.tx, [0x41; 32]);
        assert_eq!(selected.key, key);
        assert_eq!(selected.participant, DataSpaceId::new(5));
        assert_eq!(selected.value, Hash::new(&write.value));
    }
    #[test]
    fn borrowed_prepared_rows_use_canonical_sequence_and_refuse_changed_cardinality() {
        let writes = [
            ExecKv {
                key: amx_record_witness_key(AmxRecordKind::Prepared, [0x61; 32]).to_vec(),
                value: vec![3],
            },
            ExecKv {
                key: b"ordinary".to_vec(),
                value: vec![2],
            },
        ];
        let rows = RowsRef {
            writes: &writes,
            participant: DataSpaceId::new(5),
            count: count(&writes).unwrap(),
        };
        let owned = vec![row(&writes[0], DataSpaceId::new(5)).unwrap().unwrap()];
        let actual = norito::core::to_bytes(&rows).unwrap();
        let expected = norito::core::to_bytes(&owned).unwrap();
        assert_eq!(actual, expected, "one shared canonical sequence kernel");
        let changed = RowsRef { count: 2, ..rows };
        assert!(matches!(
            norito::core::to_bytes(&changed),
            Err(norito::Error::LengthMismatch)
        ));
        let malformed = [ExecKv {
            key: writes[0].key[..33].to_vec(),
            value: vec![3],
        }];
        assert!(count(&malformed).is_err());
    }
}
