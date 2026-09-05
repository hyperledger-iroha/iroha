//! Durable reservation and sender-intent recovery below the native coordinator.
//!
//! A retained intent is public retry material, never a Prepared/Committed capability or an
//! authenticated absence. All integration methods reconcile against the actual Core-owned
//! outgoing index. Hardware preparation, proofs, private inputs and terminal state stay in Core.

use super::private_journal::{PrivateJournal, PrivateJournalError, PrivateJournalFormat};
use super::*;
use norito::{Decode, Encode};
use std::{collections::BTreeMap, path::Path};

/// Maximum exact public binding admitted by one coordinator reservation.
pub const KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1: usize = 64 * 1024;
/// Maximum canonical sender intent retained before device preparation.
pub const KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1: usize = 128 * 1024;
const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "operations.norito.wal",
    magic: b"IKGOW1\0\0",
    hash_domain: b"iroha:kagemusha:v1:operation-disk:frame\0",
    maximum_payload_bytes: (KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1 + 1024) as u64,
};

/// Durable operation admission/recovery failure; no variant grants monetary authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KagemushaCoordinatorOperationStoreErrorV1 {
    /// A path, syscall or storage device is unavailable.
    #[error("coordinator operation storage unavailable")]
    StorageUnavailable,
    /// Another writer owns the same operation store.
    #[error("coordinator operation store is already open")]
    AlreadyOpen,
    /// File ownership, framing or canonical replay is invalid.
    #[error("coordinator operation journal integrity failed")]
    JournalCorrupt,
    /// A write or named-file race prevents acknowledgment of durable state.
    #[error("coordinator operation durability is uncertain")]
    DurabilityUncertain,
    /// An identifier, operation, binding, or canonical input exceeds its exact contract.
    #[error("invalid coordinator operation binding")]
    InvalidBinding,
    /// The same operation identifier was used with different immutable inputs.
    #[error("conflicting coordinator operation binding")]
    Conflict,
    /// No additional physical reservation can be admitted; existing retries remain usable.
    #[error("coordinator operation storage capacity exhausted")]
    Capacity,
    /// Restored Core state has an operation missing from or conflicting with this journal.
    #[error("coordinator operation journal disagrees with restored Core state")]
    CoreMismatch,
}

type Result<T> = core::result::Result<T, KagemushaCoordinatorOperationStoreErrorV1>;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct Reservation {
    operation_id: DigestV1,
    operation: u8,
    public_binding: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum Record {
    Initialize(KagemushaLaneIdV1),
    Reserve(Reservation),
    BeginIntent(Box<KagemushaOutgoingPublicInputPreimageV1>),
}

#[derive(Clone, Debug)]
struct RetainedOperation {
    reservation: Reservation,
    intent: Option<KagemushaOutgoingPublicInputPreimageV1>,
}

/// A real private operation WAL. Open and mutation are exposed through the Core state machine.
///
/// The byte admission budget applies only to new reservations. Reopening with a lower budget
/// retains and serves every acknowledged decision; no history count, age or budget evicts it.
/// No private witness, recovery seed, signing key or unsealed snapshot is written to this file.
pub struct KagemushaCoordinatorOperationStoreV1 {
    pub(super) wal: PrivateJournal,
    lane: KagemushaLaneIdV1,
    operations: BTreeMap<DigestV1, RetainedOperation>,
    reserved_bytes: u64,
    maximum_reserved_bytes: u64,
}

/// Sender recovery assembled from a journal intent and the actual Core-owned index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KagemushaCoordinatorSenderIntentRecoveryV1 {
    /// A reservation exists but no sender intent has been durably admitted.
    Reserved,
    /// Exact intent retained before authenticated Core preparation; no monetary phase is implied.
    Intent(KagemushaOutgoingPublicInputPreimageV1),
    /// Core's restored index owns the operation; its exact public intent also exists in the WAL.
    Indexed(KagemushaOutgoingOperationRecordV1),
}

impl KagemushaCoordinatorOperationStoreV1 {
    fn create_new(
        path: &Path,
        lane: KagemushaLaneIdV1,
        maximum_reserved_bytes: u64,
    ) -> Result<Self> {
        lane.validate().map_err(|_| Error::InvalidBinding)?;
        let wal = PrivateJournal::create_new(path, FORMAT).map_err(storage_error)?;
        let mut store = Self {
            wal,
            lane,
            operations: BTreeMap::new(),
            reserved_bytes: 0,
            maximum_reserved_bytes,
        };
        store.persist(&Record::Initialize(store.lane.clone()))?;
        Ok(store)
    }

    fn open_existing(
        path: &Path,
        lane: KagemushaLaneIdV1,
        maximum_reserved_bytes: u64,
    ) -> Result<Self> {
        lane.validate().map_err(|_| Error::InvalidBinding)?;
        let wal = PrivateJournal::open_existing(path, FORMAT).map_err(storage_error)?;
        let mut store = Self {
            wal,
            lane,
            operations: BTreeMap::new(),
            reserved_bytes: 0,
            maximum_reserved_bytes,
        };
        while let Some((sequence, payload)) = store.wal.replay_next().map_err(storage_error)? {
            let record: Record =
                norito::decode_canonical(&payload).map_err(|_| Error::JournalCorrupt)?;
            if encode(&record)? != payload {
                return Err(Error::JournalCorrupt);
            }
            if sequence == 0 {
                if record != Record::Initialize(store.lane.clone()) {
                    return Err(Error::JournalCorrupt);
                }
                continue;
            }
            match record {
                Record::Initialize(_) => return Err(Error::JournalCorrupt),
                Record::Reserve(reservation) => {
                    validate_reservation(&reservation)?;
                    if store.operations.contains_key(&reservation.operation_id) {
                        return Err(Error::JournalCorrupt);
                    }
                    store.apply_reservation(reservation)?;
                }
                Record::BeginIntent(intent) => {
                    store.require_new_intent(&intent)?;
                    store
                        .operations
                        .get_mut(&intent.operation_id)
                        .ok_or(Error::JournalCorrupt)?
                        .intent = Some(*intent);
                }
            }
        }
        Ok(store)
    }

    fn persist(&mut self, record: &Record) -> Result<()> {
        self.wal.append(&encode(record)?).map_err(storage_error)
    }

    fn apply_reservation(&mut self, reservation: Reservation) -> Result<()> {
        let growth = reservation_growth(&reservation)?;
        self.reserved_bytes = self
            .reserved_bytes
            .checked_add(growth)
            .ok_or(Error::JournalCorrupt)?;
        self.operations.insert(
            reservation.operation_id,
            RetainedOperation {
                reservation,
                intent: None,
            },
        );
        Ok(())
    }

    fn reserve(
        &mut self,
        operation_id: DigestV1,
        operation: u8,
        public_binding: &[u8],
    ) -> Result<DigestV1> {
        self.wal.check_owned().map_err(storage_error)?;
        // Check bounds before allocating a caller-controlled buffer.
        if operation_id == [0; 32]
            || !(1..=22).contains(&operation)
            || public_binding.is_empty()
            || public_binding.len() > KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1
        {
            return Err(Error::InvalidBinding);
        }
        if let Some(existing) = self.operations.get(&operation_id) {
            return if existing.reservation.operation == operation
                && existing.reservation.public_binding == public_binding
            {
                Ok(operation_id)
            } else {
                Err(Error::Conflict)
            };
        }
        let reservation = Reservation {
            operation_id,
            operation,
            public_binding: public_binding.to_vec(),
        };
        validate_reservation(&reservation)?;
        let growth = reservation_growth(&reservation)?;
        if self
            .reserved_bytes
            .checked_add(growth)
            .is_none_or(|next| next > self.maximum_reserved_bytes)
        {
            return Err(Error::Capacity);
        }
        self.persist(&Record::Reserve(reservation.clone()))?;
        self.apply_reservation(reservation)?;
        Ok(operation_id)
    }

    fn require_new_intent(&self, intent: &KagemushaOutgoingPublicInputPreimageV1) -> Result<()> {
        validate_intent(intent, &self.lane)?;
        let retained = self
            .operations
            .get(&intent.operation_id)
            .ok_or(Error::Conflict)?;
        if retained.reservation.operation != 5
            || retained.reservation.public_binding != sender_public_binding(&intent.inputs)?
        {
            return Err(Error::Conflict);
        }
        if retained.intent.is_some() {
            return Err(Error::Conflict);
        }
        Ok(())
    }

    fn begin(&mut self, intent: &KagemushaOutgoingPublicInputPreimageV1) -> Result<()> {
        self.wal.check_owned().map_err(storage_error)?;
        if let Some(existing) = self
            .operations
            .get(&intent.operation_id)
            .and_then(|entry| entry.intent.as_ref())
        {
            return if existing == intent {
                Ok(())
            } else {
                Err(Error::Conflict)
            };
        }
        self.require_new_intent(intent)?;
        self.persist(&Record::BeginIntent(Box::new(intent.clone())))?;
        self.operations
            .get_mut(&intent.operation_id)
            .ok_or(Error::JournalCorrupt)?
            .intent = Some(intent.clone());
        Ok(())
    }
}

use KagemushaCoordinatorOperationStoreErrorV1 as Error;

fn encode(record: &Record) -> Result<Vec<u8>> {
    norito::encode_canonical(record).map_err(|_| Error::InvalidBinding)
}

fn validate_reservation(reservation: &Reservation) -> Result<()> {
    if reservation.operation_id == [0; 32]
        || !(1..=22).contains(&reservation.operation)
        || reservation.public_binding.is_empty()
        || reservation.public_binding.len() > KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1
    {
        return Err(Error::InvalidBinding);
    }
    if reservation.operation == 5 {
        let inputs: KagemushaOutgoingPublicInputsV1 =
            norito::decode_canonical(&reservation.public_binding)
                .map_err(|_| Error::InvalidBinding)?;
        if sender_public_binding(&inputs)? != reservation.public_binding {
            return Err(Error::InvalidBinding);
        }
        match inputs {
            KagemushaOutgoingPublicInputsV1::SendSplit { request } if request.is_empty() => {
                return Err(Error::InvalidBinding);
            }
            KagemushaOutgoingPublicInputsV1::RedeemSplit { amount: 0, .. } => {
                return Err(Error::InvalidBinding);
            }
            _ => {}
        }
    }
    Ok(())
}

fn reservation_growth(reservation: &Reservation) -> Result<u64> {
    validate_reservation(reservation)?;
    let bytes = encode(&Record::Reserve(reservation.clone()))?.len() as u64
        + private_journal::FRAME_HEADER_BYTES as u64;
    // Reserve the full bounded future Intent append before accepting any sender work.
    let intent = if reservation.operation == 5 {
        KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1 as u64 + 1024
    } else {
        0
    };
    bytes.checked_add(intent).ok_or(Error::Capacity)
}

fn validate_intent(
    intent: &KagemushaOutgoingPublicInputPreimageV1,
    lane: &KagemushaLaneIdV1,
) -> Result<()> {
    if &intent.context.lane != lane {
        return Err(Error::InvalidBinding);
    }
    if let KagemushaOutgoingPublicInputsV1::SendSplit { request } = &intent.inputs {
        if request.len() > KAGEMUSHA_COORDINATOR_PUBLIC_BINDING_MAX_BYTES_V1 {
            return Err(Error::InvalidBinding);
        }
    }
    intent
        .canonical_digest()
        .map_err(|_| Error::InvalidBinding)?;
    if norito::encode_canonical(intent)
        .map_err(|_| Error::InvalidBinding)?
        .len()
        > KAGEMUSHA_COORDINATOR_INTENT_MAX_BYTES_V1
    {
        return Err(Error::InvalidBinding);
    }
    Ok(())
}

/// Canonical tagged sender inputs shared by Core and the device bridge. The enum tag is required
/// because SendSplit and RedeemSplit use the same device operation code.
fn sender_public_binding(inputs: &KagemushaOutgoingPublicInputsV1) -> Result<Vec<u8>> {
    norito::encode_canonical(inputs).map_err(|_| Error::InvalidBinding)
}

fn storage_error(error: PrivateJournalError) -> Error {
    match error {
        PrivateJournalError::StorageUnavailable => Error::StorageUnavailable,
        PrivateJournalError::AlreadyOpen => Error::AlreadyOpen,
        PrivateJournalError::Corrupt => Error::JournalCorrupt,
        PrivateJournalError::Uncertain => Error::DurabilityUncertain,
    }
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    /// Create new coordinator retry storage for this actual Core wallet. Existing files never reset.
    /// An existing Core operation requires an existing matching store, not a new empty journal.
    pub fn create_coordinator_operation_store(
        &self,
        path: &Path,
        maximum_reserved_bytes: u64,
    ) -> Result<KagemushaCoordinatorOperationStoreV1> {
        if !self.outgoing_operation_index().is_empty() {
            return Err(Error::CoreMismatch);
        }
        KagemushaCoordinatorOperationStoreV1::create_new(
            path,
            self.state.lane.clone(),
            maximum_reserved_bytes,
        )
    }

    /// Open the existing journal and reconcile every retained Core operation before serving it.
    pub fn open_coordinator_operation_store(
        &self,
        path: &Path,
        maximum_reserved_bytes: u64,
    ) -> Result<KagemushaCoordinatorOperationStoreV1> {
        let store = KagemushaCoordinatorOperationStoreV1::open_existing(
            path,
            self.state.lane.clone(),
            maximum_reserved_bytes,
        )?;
        self.reconcile_coordinator_operations(&store)?;
        Ok(store)
    }

    /// Admit a caller-persisted ID only after its exact operation/binding is durably retained.
    /// The returned ID equals the caller's ID and confers no qualification or monetary capability.
    pub fn reserve_coordinator_operation(
        &self,
        store: &mut KagemushaCoordinatorOperationStoreV1,
        operation_id: DigestV1,
        operation: u8,
        public_binding: &[u8],
    ) -> Result<DigestV1> {
        self.reconcile_coordinator_operations(store)?;
        store.reserve(operation_id, operation, public_binding)
    }

    /// Durably bind one sender intent before hardware preparation. Exact retries do not append.
    /// Credential identity must already be authenticated by the native owner, as for existing Core
    /// preparation APIs; a raw credential ID or this return value is never an authentication token.
    pub fn begin_coordinator_sender_intent(
        &self,
        store: &mut KagemushaCoordinatorOperationStoreV1,
        intent: &KagemushaOutgoingPublicInputPreimageV1,
    ) -> Result<()> {
        self.reconcile_coordinator_operations(store)?;
        validate_intent(intent, &self.state.lane)?;
        let existing = self
            .classify_outgoing_operation_prepare(intent)
            .map_err(|_| Error::Conflict)?;
        if existing.is_none()
            && (intent.context.release != self.state.context()
                || intent.context.hardware_epoch != self.state.hardware_epoch
                || intent.context.device_policy_binding != self.state.device_policy_binding)
        {
            return Err(Error::InvalidBinding);
        }
        store.begin(intent)
    }

    /// Recover a reserved sender's exact intent together with its actual Core index projection.
    /// Missing reservation is a conflict, not the bridge's authenticated-absence response.
    pub fn recover_coordinator_sender_intent(
        &self,
        store: &KagemushaCoordinatorOperationStoreV1,
        operation_id: DigestV1,
    ) -> Result<KagemushaCoordinatorSenderIntentRecoveryV1> {
        self.reconcile_coordinator_operations(store)?;
        let retained = store.operations.get(&operation_id).ok_or(Error::Conflict)?;
        if retained.reservation.operation != 5 {
            return Err(Error::InvalidBinding);
        }
        let Some(intent) = &retained.intent else {
            return Ok(KagemushaCoordinatorSenderIntentRecoveryV1::Reserved);
        };
        match self
            .classify_outgoing_operation_prepare(intent)
            .map_err(|_| Error::CoreMismatch)?
        {
            Some(record) => Ok(KagemushaCoordinatorSenderIntentRecoveryV1::Indexed(
                record.clone(),
            )),
            None => Ok(KagemushaCoordinatorSenderIntentRecoveryV1::Intent(
                intent.clone(),
            )),
        }
    }

    fn reconcile_coordinator_operations(
        &self,
        store: &KagemushaCoordinatorOperationStoreV1,
    ) -> Result<()> {
        store.wal.check_owned().map_err(storage_error)?;
        if store.lane != self.state.lane {
            return Err(Error::CoreMismatch);
        }
        // Full coverage matters: an older valid journal prefix must not hide a different Core
        // operation merely because the caller happens to request an unrelated ID this time.
        for record in self.outgoing_operation_index().records() {
            let intent = store
                .operations
                .get(&record.operation_id)
                .and_then(|entry| entry.intent.as_ref())
                .ok_or(Error::CoreMismatch)?;
            let matched = self
                .classify_outgoing_operation_prepare(intent)
                .map_err(|_| Error::CoreMismatch)?;
            if matched != Some(record) {
                return Err(Error::CoreMismatch);
            }
        }
        Ok(())
    }
}
