//! Shared bounded custody history for the two native final-promotion purposes.
//!
//! This crate-private owner authenticates retained control rows, not consensus finality or
//! present hardware use. Purpose adapters retain distinct namespaces, records and domains.

use crate::state::{StateReadOnly, StateTransaction, WorldReadOnly};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::account::AccountId;
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    custody_control::SignerCustodyControlStateV1,
    protocol::{SIGNER_MAX_ID_BYTES_V1, SignerPurposeBindingV1, SignerRoleV1},
};

mod purpose;
mod read;
#[cfg(test)]
pub(crate) mod staging_fixture;
mod transition;
pub(crate) use purpose::{AccountPurpose, ReceiptPurpose};
#[cfg(test)]
pub(crate) use read::read_control_record;
pub(crate) use read::{control_digest, read_control, read_control_at};
pub(crate) use transition::{ControlAction, ControlTransition, control_revision, prepare_control};

const MAX_FRAME_BYTES_V1: usize = 32 * 1024;

/// Payload-free retained-control failure, mapped once by each public purpose owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryError {
    Invalid,
    BindingMismatch,
    CorruptHistory,
    HeightUnavailable,
    Conflict,
    Capacity,
    Generation,
    Custody,
}
impl std::fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "native custody history rejected: {self:?}")
    }
}
impl std::error::Error for HistoryError {}

mod sealed {
    pub trait Purpose {}
    pub trait Execution {}
}

/// Borrowed execution coordinates; never an independently trusted execution assertion.
#[derive(Clone, Copy)]
pub(crate) struct ExecutionView<'a> {
    pub(crate) height: u64,
    pub(crate) ordinal: u32,
    pub(crate) recorded_at_unix_ms: u64,
    pub(crate) authority: &'a AccountId,
}
/// The existing distinct execution DTOs share only their deterministic coordinate rules.
pub(crate) trait CustodyExecution: sealed::Execution {
    fn view(&self) -> ExecutionView<'_>;
    fn build(value: ExecutionView<'_>) -> Self;
}
/// Borrowed fields of one purpose-owned immutable record.
pub(crate) struct ControlRecordView<'a, E> {
    pub(crate) deployment: &'a str,
    pub(crate) revision: u64,
    pub(crate) predecessor_digest: [u8; 32],
    pub(crate) request_digest: [u8; 32],
    pub(crate) execution: &'a E,
    pub(crate) control_state: &'a [u8],
    pub(crate) enrollment: Option<&'a [u8]>,
}
/// Owned fields prepared only after the common native transition checks.
pub(crate) struct ControlRecordParts<E> {
    pub(crate) deployment: String,
    pub(crate) revision: u64,
    pub(crate) predecessor_digest: [u8; 32],
    pub(crate) request_digest: [u8; 32],
    pub(crate) execution: E,
    pub(crate) control_state: Vec<u8>,
    pub(crate) enrollment: Option<Vec<u8>>,
}
/// Closed purpose selection, with no caller-supplied namespace, verifier or authority callback.
pub(crate) trait CustodyPurpose: sealed::Purpose {
    type Record: Clone
        + PartialEq
        + norito::core::NoritoSerialize
        + for<'de> norito::core::NoritoDeserialize<'de>;
    type Execution: CustodyExecution;
    const NAMESPACE: &'static str;
    const RECORD_DOMAIN: &'static [u8];
    const ROLE: SignerRoleV1;
    const MAX_REVISIONS: u64;
    const NORMAL_REVISIONS: u64;
    fn purpose(deployment: String) -> SignerPurposeBindingV1;
    fn record_view(record: &Self::Record) -> ControlRecordView<'_, Self::Execution>;
    fn build_record(parts: ControlRecordParts<Self::Execution>) -> Self::Record;
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
#[norito_schema(name = "iroha_core::query::final_promotion_authority::ControlIndexV1")]
pub(crate) struct ControlIndexV1 {
    pub(crate) revision: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) height: u64,
    pub(crate) ordinal: u32,
}
/// Internally validated control and its immutable purpose-owned row/index.
pub(crate) struct NativeControl<P: CustodyPurpose> {
    pub(crate) record: P::Record,
    pub(crate) state: SignerCustodyControlStateV1,
    pub(crate) index: ControlIndexV1,
}

pub(crate) fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Result<Vec<u8>, HistoryError> {
    if norito::canonical_frame_len(value).map_err(|_| HistoryError::Invalid)? > MAX_FRAME_BYTES_V1 {
        return Err(HistoryError::Invalid);
    }
    norito::encode_canonical(value).map_err(|_| HistoryError::Invalid)
}
pub(crate) fn decode<T>(bytes: &[u8]) -> Result<T, HistoryError>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > MAX_FRAME_BYTES_V1 {
        return Err(HistoryError::Invalid);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            MAX_FRAME_BYTES_V1,
            MAX_FRAME_BYTES_V1,
            2 * MAX_FRAME_BYTES_V1,
            256 * 1024,
            16,
        ),
    )
    .map_err(|_| HistoryError::Invalid)
}
pub(crate) fn digest<T: norito::core::NoritoSerialize>(
    domain: &[u8],
    value: &T,
) -> Result<[u8; 32], HistoryError> {
    let frame = encode(value)?;
    let mut bytes = Vec::with_capacity(domain.len() + frame.len());
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&frame);
    Ok(*Hash::new(bytes).as_ref())
}
pub(crate) fn valid_deployment<P: CustodyPurpose>(deployment: &str) -> bool {
    !deployment.is_empty()
        && deployment.len() <= SIGNER_MAX_ID_BYTES_V1
        && P::purpose(deployment.to_owned()).validates_role(P::ROLE)
}
pub(crate) fn scope<P: CustodyPurpose>(deployment: &str) -> String {
    format!(
        "{}_{}",
        P::NAMESPACE,
        hex::encode(Hash::new(deployment.as_bytes()).as_ref())
    )
}
pub(crate) fn path<P: CustodyPurpose>(
    deployment: &str,
    suffix: &str,
) -> Result<StatePath, HistoryError> {
    if !valid_deployment::<P>(deployment) {
        return Err(HistoryError::BindingMismatch);
    }
    format!("{}_{}", scope::<P>(deployment), suffix)
        .parse()
        .map_err(|_| HistoryError::Invalid)
}
pub(crate) fn control_head_key<P: CustodyPurpose>(
    deployment: &str,
) -> Result<StatePath, HistoryError> {
    path::<P>(deployment, "control_head")
}
pub(crate) fn control_record_key<P: CustodyPurpose>(
    deployment: &str,
    revision: u64,
) -> Result<StatePath, HistoryError> {
    path::<P>(deployment, &format!("control_revision_{revision:020}"))
}
pub(crate) fn control_height_key<P: CustodyPurpose>(
    deployment: &str,
    height: u64,
    ordinal: u32,
) -> Result<StatePath, HistoryError> {
    path::<P>(
        deployment,
        &format!("control_height_{height:020}_{ordinal:010}"),
    )
}
pub(crate) fn key_path<P: CustodyPurpose>(
    deployment: &str,
    signer: bool,
    key: &PublicKey,
) -> Result<StatePath, HistoryError> {
    path::<P>(
        deployment,
        &format!(
            "{}_key_{}",
            if signer { "signer" } else { "attester" },
            hex::encode(Hash::new(encode(key)?).as_ref())
        ),
    )
}
pub(crate) fn prefix_has_any<P: CustodyPurpose>(
    world: &impl WorldReadOnly,
    deployment: &str,
    suffix: &str,
) -> Result<bool, HistoryError> {
    let start = path::<P>(deployment, suffix)?;
    Ok(world
        .smart_contract_state()
        .range(start.clone()..)
        .next()
        .is_some_and(|(key, _)| key.as_ref().starts_with(start.as_ref())))
}
pub(crate) fn valid_execution<E: CustodyExecution>(execution: &E) -> bool {
    let value = execution.view();
    value.height != 0 && value.recorded_at_unix_ms != 0 && value.recorded_at_unix_ms != u64::MAX
}
pub(crate) fn adjacent_execution<E: CustodyExecution>(
    old: &E,
    next: &E,
) -> Result<(), HistoryError> {
    let old = old.view();
    let next = next.view();
    let ordinal = if old.height == next.height {
        old.ordinal.checked_add(1).ok_or(HistoryError::Capacity)?
    } else {
        0
    };
    if next.height == 0
        || next.recorded_at_unix_ms == 0
        || next.recorded_at_unix_ms == u64::MAX
        || next.height < old.height
        || next.ordinal != ordinal
        || next.recorded_at_unix_ms < old.recorded_at_unix_ms
        || (next.height == old.height && next.recorded_at_unix_ms != old.recorded_at_unix_ms)
    {
        return Err(HistoryError::CorruptHistory);
    }
    Ok(())
}
pub(crate) fn execution<E: CustodyExecution>(
    tx: &StateTransaction<'_, '_>,
    authority: &AccountId,
    previous: Option<&E>,
) -> Result<E, HistoryError> {
    let height = u64::try_from(tx.block_hashes().len())
        .map_err(|_| HistoryError::HeightUnavailable)?
        .checked_add(1)
        .ok_or(HistoryError::HeightUnavailable)?;
    let now = tx.block_unix_timestamp_ms();
    if height != tx._curr_block.height().get() || now == 0 || now == u64::MAX {
        return Err(HistoryError::Invalid);
    }
    let ordinal = match previous.map(CustodyExecution::view) {
        Some(old) if old.height == height => {
            old.ordinal.checked_add(1).ok_or(HistoryError::Capacity)?
        }
        _ => 0,
    };
    let next = E::build(ExecutionView {
        height,
        ordinal,
        recorded_at_unix_ms: now,
        authority,
    });
    if let Some(old) = previous {
        adjacent_execution(old, &next)?;
    }
    Ok(next)
}
pub(crate) fn validate_binding<P: CustodyPurpose>(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
    deployment: &str,
) -> Result<(), HistoryError> {
    binding.validate().map_err(|_| HistoryError::Invalid)?;
    if binding.chain_id != state.chain_id().to_string()
        || binding.network_id != *state.network_id().as_bytes()
        || binding.role != P::ROLE
        || binding.purpose != P::purpose(deployment.to_owned())
    {
        return Err(HistoryError::BindingMismatch);
    }
    Ok(())
}
/// Match actual committed predecessor control without fabricating its block anchor.
pub(crate) fn committed_control<P: CustodyPurpose>(
    tx: &StateTransaction<'_, '_>,
    current: &NativeControl<P>,
) -> Result<SignerCustodyAnchorV1, HistoryError> {
    let parent =
        u64::try_from(tx.block_hashes().len()).map_err(|_| HistoryError::HeightUnavailable)?;
    let deployment = P::record_view(&current.record).deployment;
    let selected =
        read_control_at::<P>(tx.world(), deployment, parent)?.ok_or(HistoryError::Conflict)?;
    if selected.index != current.index
        || selected.state != current.state
        || selected.record != current.record
    {
        return Err(HistoryError::Conflict);
    }
    validate_binding::<P>(tx, &selected.state.policy.binding, deployment)?;
    let offset = parent
        .checked_sub(1)
        .and_then(|height| usize::try_from(height).ok())
        .ok_or(HistoryError::HeightUnavailable)?;
    let block_hash = tx
        .block_hashes()
        .get(offset)
        .ok_or(HistoryError::HeightUnavailable)?;
    Ok(SignerCustodyAnchorV1 {
        height: parent,
        block_hash: *block_hash.as_ref(),
        state_digest: selected.index.digest,
    })
}
/// Stage immutable bytes only; the native purpose owner publishes after all extra checks succeed.
pub(crate) fn immutable(
    world: &impl WorldReadOnly,
    writes: &mut Vec<(StatePath, Vec<u8>)>,
    key: StatePath,
    bytes: Vec<u8>,
) -> Result<(), HistoryError> {
    if world.smart_contract_state().get(&key).is_some() || writes.iter().any(|(old, _)| old == &key)
    {
        return Err(HistoryError::CorruptHistory);
    }
    writes.push((key, bytes));
    Ok(())
}
