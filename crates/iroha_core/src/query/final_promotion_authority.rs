//! Bounded same-State deployment custody and durable signer-operation authority.
//!
//! Native rows contain execution coordinates, never their own block hash. These readers attach
//! hashes from the exact committed State view; callers must additionally authenticate Kura/QC
//! finality. Custody and operation journals have independent digests and immutable indexes.

use crate::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::sorafs::final_promotion_authority::{
    FINAL_PROMOTION_MAX_OPERATIONS_V1, FinalPromotionCustodyRecordV1, FinalPromotionExecutionV1,
    FinalPromotionOperationOutcomeV1, FinalPromotionOperationRecordV1,
};
use iroha_data_model::{account::AccountId, isi::sorafs::MutateSorafsFinalPromotionAuthority};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::{SignerOperationAuditHeadV1, SignerPurposeBindingV1},
};

pub(crate) mod check;
pub mod observation;
pub(crate) mod operation;
use crate::query::signer_custody_history as history;
use crate::query::signer_custody_history::{HistoryError, ReceiptPurpose, read_control_at};
pub(crate) use operation::{read_operation_head, read_operation_record, read_operation_slot};

/// Payload-free native admission and retained-state failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalPromotionAuthorityErrorV1 {
    /// Malformed, oversized or noncanonical public data.
    Invalid,
    /// Chain, network, deployment, role, binding or permission mismatch.
    BindingMismatch,
    /// Missing or inconsistent retained rows and indexes.
    CorruptHistory,
    /// The requested committed block is unavailable in this State.
    HeightUnavailable,
    /// An exact predecessor, reservation or retry expectation differs.
    Conflict,
    /// Finite retained history or fencing capacity is exhausted.
    Capacity,
    /// Key, policy or enrollment generation would roll back or reuse retired authority.
    Generation,
    /// Independent signer authorization verification failed.
    Custody,
    /// The reservation is not eligible at the deterministic execution time.
    ReservationTime,
}
impl std::fmt::Display for FinalPromotionAuthorityErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "final-promotion authority rejected: {self:?}")
    }
}
impl std::error::Error for FinalPromotionAuthorityErrorV1 {}
pub(crate) use FinalPromotionAuthorityErrorV1 as Error;
impl From<HistoryError> for FinalPromotionAuthorityErrorV1 {
    fn from(error: HistoryError) -> Self {
        match error {
            HistoryError::Invalid => Self::Invalid,
            HistoryError::BindingMismatch => Self::BindingMismatch,
            HistoryError::CorruptHistory => Self::CorruptHistory,
            HistoryError::HeightUnavailable => Self::HeightUnavailable,
            HistoryError::Conflict => Self::Conflict,
            HistoryError::Capacity => Self::Capacity,
            HistoryError::Generation => Self::Generation,
            HistoryError::Custody => Self::Custody,
        }
    }
}

/// Compute the exact native authority request identity used by retained mutation records.
///
/// This binds the complete deployment, control CAS, action payload and canonical account under
/// the native request domain. It does not establish execution, authorization, custody or freshness;
/// callers must authenticate the matching retained row separately. Check actions store no such row.
///
/// # Errors
/// Rejects canonical encoding failures or an instruction/account frame larger than the native
/// [`iroha_data_model::sorafs::final_promotion_authority::FINAL_PROMOTION_MAX_RECORD_BYTES_V1`]
/// bound, before allocating that encoded frame.
pub fn final_promotion_authority_request_digest_v1(
    instruction: &MutateSorafsFinalPromotionAuthority,
    authority: &AccountId,
) -> Result<[u8; 32], FinalPromotionAuthorityErrorV1> {
    let mut bytes = b"iroha.sorafs.final-promotion.authority-request.v1\0".to_vec();
    bytes.extend_from_slice(&encode(instruction)?);
    bytes.extend_from_slice(&encode(authority)?);
    Ok(*Hash::new(bytes).as_ref())
}

/// Native operation journal head, independent of the custody control digest.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_core::query::final_promotion_authority::FinalPromotionOperationHeadV1"
)]
pub struct FinalPromotionOperationHeadV1 {
    /// Last immutable global operation journal revision, or zero before the first reservation.
    pub revision: u64,
    /// Exact digest of that revision, or zero before the first reservation.
    pub digest: [u8; 32],
    /// Last allocated fencing generation, preserved across every custody rotation.
    pub fence: u64,
    /// Last successfully completed audit successor, preserved across custody rotation.
    pub audit: SignerOperationAuditHeadV1,
    /// Sole live slot; spent operation identities remain in their immutable retained indexes.
    pub active_operation: Option<[u8; 32]>,
    /// Number of distinct admitted operation identities, including failed reservations.
    pub total_admissions: u64,
}
impl FinalPromotionOperationHeadV1 {
    pub(crate) const fn empty() -> Self {
        Self {
            revision: 0,
            digest: [0; 32],
            fence: 0,
            audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
            active_operation: None,
            total_admissions: 0,
        }
    }
}

/// Runtime-only coherent custody/operation snapshot; this is not signed observation evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalPromotionAuthoritySnapshotV1 {
    /// Exact selected native custody record, including request identity and original authority.
    pub control_record: FinalPromotionCustodyRecordV1,
    /// Selected governed policy, enrollment head and revocations.
    pub control: SignerCustodyControlStateV1,
    /// Requested committed block hash paired with its selected native custody digest.
    pub custody_anchor: SignerCustodyAnchorV1,
    /// Operation journal head selected at the same committed block.
    pub operations: FinalPromotionOperationHeadV1,
    /// Latest revision for the requested operation at that block, if any.
    pub operation: Option<FinalPromotionOperationRecordV1>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::codec::Encode,
    norito::codec::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::query::final_promotion_authority::OperationIndexV1")]
pub(crate) struct OperationIndexV1 {
    pub(crate) head: FinalPromotionOperationHeadV1,
    pub(crate) height: u64,
    pub(crate) ordinal: u32,
}
pub(crate) struct NativeOperation {
    pub(crate) record: FinalPromotionOperationRecordV1,
    pub(crate) index: OperationIndexV1,
}

pub(crate) fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    history::encode(value).map_err(Into::into)
}
pub(crate) fn decode<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    history::decode(bytes).map_err(Into::into)
}
pub(crate) fn digest<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], Error> {
    history::digest(domain, value).map_err(Into::into)
}
pub(crate) fn valid_deployment(deployment: &str) -> bool {
    history::valid_deployment::<ReceiptPurpose>(deployment)
}
pub(crate) fn path(deployment: &str, suffix: &str) -> Result<StatePath, Error> {
    history::path::<ReceiptPurpose>(deployment, suffix).map_err(Into::into)
}
pub(crate) fn operation_head_key(deployment: &str) -> Result<StatePath, Error> {
    path(deployment, "operation_head")
}
pub(crate) fn operation_record_key(deployment: &str, revision: u64) -> Result<StatePath, Error> {
    path(deployment, &format!("operation_revision_{revision:020}"))
}
pub(crate) fn operation_height_key(
    deployment: &str,
    height: u64,
    ordinal: u32,
) -> Result<StatePath, Error> {
    path(
        deployment,
        &format!("operation_height_{height:020}_{ordinal:010}"),
    )
}
pub(crate) fn operation_slot_key(deployment: &str, id: [u8; 32]) -> Result<StatePath, Error> {
    path(deployment, &format!("operation_id_{}", hex::encode(id)))
}
pub(crate) fn operation_admission_key(deployment: &str, id: [u8; 32]) -> Result<StatePath, Error> {
    path(
        deployment,
        &format!("operation_admission_{}", hex::encode(id)),
    )
}
pub(crate) fn prefix_has_any(
    world: &impl WorldReadOnly,
    deployment: &str,
    suffix: &str,
) -> Result<bool, Error> {
    history::prefix_has_any::<ReceiptPurpose>(world, deployment, suffix).map_err(Into::into)
}
pub(crate) fn valid_execution(execution: &FinalPromotionExecutionV1) -> bool {
    history::valid_execution(execution)
}
pub(crate) fn adjacent_execution(
    old: &FinalPromotionExecutionV1,
    next: &FinalPromotionExecutionV1,
) -> Result<(), Error> {
    history::adjacent_execution(old, next).map_err(Into::into)
}
pub(crate) fn validate_binding(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    deployment: &str,
) -> Result<(), Error> {
    history::validate_binding::<ReceiptPurpose>(state, binding, deployment).map_err(Into::into)
}

/// Read exact deployment custody, operation head and optional operation from one committed State.
///
/// # Errors
/// Rejects missing blocks, substituted scopes/bindings and inconsistent retained history.
pub fn read_final_promotion_authority_at_v1(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    height: u64,
    operation_id: Option<[u8; 32]>,
) -> Result<Option<FinalPromotionAuthoritySnapshotV1>, Error> {
    let SignerPurposeBindingV1::FinalPromotionProvenance { deployment_id } = &binding.purpose
    else {
        return Err(Error::BindingMismatch);
    };
    validate_binding(state, binding, deployment_id)?;
    let offset = height
        .checked_sub(1)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(Error::HeightUnavailable)?;
    let block_hash = state
        .block_hashes()
        .get(offset)
        .ok_or(Error::HeightUnavailable)?;
    let operations = operation::read_operation_head_at(state.world(), deployment_id, height)?;
    let Some(control) = read_control_at::<ReceiptPurpose>(state.world(), deployment_id, height)?
    else {
        if operations.revision != 0 {
            return Err(Error::CorruptHistory);
        }
        return Ok(None);
    };
    if control.state.policy.binding != *binding {
        return Err(Error::BindingMismatch);
    }
    let active = operations
        .active_operation
        .map(|id| operation::read_operation_at(state.world(), deployment_id, id, height))
        .transpose()?
        .flatten();
    if operations.active_operation.is_some() {
        let active = active.as_ref().ok_or(Error::CorruptHistory)?;
        // A control transition terminalizes its active slot atomically. Independently valid
        // journal prefixes must not produce a newer control paired with an older live slot.
        if active.outcome != FinalPromotionOperationOutcomeV1::Reserved
            || active.revision != operations.revision
            || operation::operation_digest(active)? != operations.digest
            || active.custody.control_state_digest != control.index.digest
            || control
                .state
                .active_head
                .is_none_or(|head| head.record_digest != active.custody.record_digest)
            || control.state.signer_revoked
            || control.state.attester_revoked
        {
            return Err(Error::CorruptHistory);
        }
    }
    let operation = match operation_id {
        Some(id) if Some(id) == operations.active_operation => active,
        Some(id) => operation::read_operation_at(state.world(), deployment_id, id, height)?,
        None => None,
    };
    Ok(Some(FinalPromotionAuthoritySnapshotV1 {
        control_record: control.record,
        control: control.state,
        custody_anchor: SignerCustodyAnchorV1 {
            height,
            block_hash: *block_hash.as_ref(),
            state_digest: control.index.digest,
        },
        operations,
        operation,
    }))
}
