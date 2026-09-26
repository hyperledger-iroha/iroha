//! Provider-scoped retained role-11 operation rows and permanent admission indexes.
//!
//! This reader authenticates the adjacent immutable State history only. Kura/QC finality and
//! successful execution proofs remain a separate, currently closed Check gate.

use crate::state::WorldReadOnly;
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenNativeOperationV1, StreamTokenOutcomeV1,
            validate_stream_token_native_operation_claim_v1,
        },
    },
};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use norito::codec::{Decode, Encode};
use sorafs_manifest::signer::protocol::SignerOperationAuditHeadV1;
use std::str::FromStr;

/// Maximum number of unique operation IDs admitted by one provider in V1.
pub const STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1: u64 = 65_536;
/// Maximum lifetime of one exclusive native reservation before earlier token/policy expiry.
pub const STREAM_TOKEN_NATIVE_RESERVATION_MS_V1: u64 = 60_000;
const MAX_REVISIONS: u64 = STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1 * 2;
const MAX_ROW_BYTES: usize = 24 * 1024;
const RECORD_DOMAIN: &[u8] = b"iroha.sorafs.stream-token.operation-record.v1\0";

/// Payload-free native operation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamTokenAuthorityErrorV1 {
    /// Malformed or oversized canonical data.
    Invalid,
    /// Provider, custody, signed authority or network scope differs.
    BindingMismatch,
    /// A retained row or index disagrees with its immutable predecessor.
    CorruptHistory,
    /// Exact CAS, spent ID or active slot differs.
    Conflict,
    /// Bounded operation or retained-row capacity is exhausted.
    Capacity,
    /// Current custody or reservation is not eligible at the execution time.
    Custody,
    /// Exact direct signed execution context is unavailable.
    Execution,
    /// Bounded historical evidence or finalized challenged Check is unavailable.
    CheckUnavailable,
    /// Exact signed-RS16 history, Kura/QC association or floor continuity failed.
    Finality,
}
impl std::fmt::Display for StreamTokenAuthorityErrorV1 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "stream-token native authority rejected: {self:?}")
    }
}
impl std::error::Error for StreamTokenAuthorityErrorV1 {}
pub(crate) use StreamTokenAuthorityErrorV1 as Error;

/// Latest immutable per-provider journal identity and original active slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_authority::OperationHeadV1")]
pub struct OperationHeadV1 {
    /// Last immutable revision, zero before first Reserve.
    pub revision: u64,
    /// Domain-separated canonical digest of that revision.
    pub digest: [u8; 32],
    /// Never-reused reservation fence.
    pub fence: u64,
    /// Last completed signer audit successor.
    pub audit: SignerOperationAuditHeadV1,
    /// Sole active original operation ID, if any.
    pub active_operation: Option<[u8; 32]>,
    /// Number of unique admitted IDs, equal to the fence.
    pub total_admissions: u64,
}
impl OperationHeadV1 {
    /// Empty provider operation journal before its first Reserve.
    #[must_use]
    pub const fn empty() -> Self {
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

/// One immutable Reserve or terminal transition in the native State journal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::query::stream_token_authority::OperationRecordV1")]
pub struct OperationRecordV1 {
    /// Strict adjacent provider journal revision.
    pub revision: u64,
    /// Exact digest of the preceding immutable row, zero only at revision one.
    pub predecessor_digest: [u8; 32],
    /// Domain-separated canonical signed instruction and authority identity.
    pub request_digest: [u8; 32],
    /// Original reviewed request and exact native Reserve/terminal execution coordinates.
    pub operation: StreamTokenNativeOperationV1,
}

/// Exact original Reserve and current row for one permanently admitted operation ID.
///
/// This is only a same-State history pair. It does not authenticate either signed transaction,
/// successful output, Kura/QC finality, or a challenged Check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OperationHistoryV1 {
    /// The original immutable first-use Reserve record.
    pub(crate) reserved: OperationRecordV1,
    /// The original Reserve or its immediately adjacent terminal record.
    pub(crate) current: OperationRecordV1,
}

/// Bound canonical Norito encoding for a role-11 State row or index.
pub(crate) fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    if norito::canonical_frame_len(value).map_err(|_| Error::Invalid)? > MAX_ROW_BYTES {
        return Err(Error::Invalid);
    }
    norito::encode_canonical(value).map_err(|_| Error::Invalid)
}

/// Decode one bounded canonical role-11 State row or index.
pub(crate) fn decode<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > MAX_ROW_BYTES {
        return Err(Error::Invalid);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            MAX_ROW_BYTES,
            MAX_ROW_BYTES,
            2 * MAX_ROW_BYTES,
            256 * 1024,
            16,
        ),
    )
    .map_err(|_| Error::Invalid)
}

/// Exact immutable native row commitment.
pub(crate) fn record_digest(record: &OperationRecordV1) -> Result<[u8; 32], Error> {
    let frame = encode(record)?;
    let mut preimage = Vec::with_capacity(RECORD_DOMAIN.len() + frame.len());
    preimage.extend_from_slice(RECORD_DOMAIN);
    preimage.extend_from_slice(&frame);
    Ok(*Hash::new(preimage).as_ref())
}

/// Domain-separated exact direct role-11 instruction and signing-account identity.
pub(crate) fn request_digest(
    instruction: &MutateSorafsStreamTokenAuthority,
    authority: &AccountId,
) -> Result<[u8; 32], Error> {
    let request = encode(instruction)?;
    let account = encode(authority)?;
    let mut bytes = Vec::with_capacity(request.len() + account.len() + 64);
    bytes.extend_from_slice(b"iroha.sorafs.stream-token.authority-request.v1\0");
    bytes.extend_from_slice(&request);
    bytes.extend_from_slice(&account);
    Ok(*Hash::new(bytes).as_ref())
}

fn scope(provider: ProviderId) -> String {
    format!(
        "sorafs_stream_token_operation_v1_{}",
        hex::encode(provider.as_bytes())
    )
}
fn path(provider: ProviderId, suffix: &str) -> StatePath {
    StatePath::from_str(&format!("{}_{}", scope(provider), suffix))
        .expect("bounded provider-scoped native operation path")
}
fn prefix_has_any(world: &impl WorldReadOnly, provider: ProviderId, suffix: &str) -> bool {
    let prefix = path(provider, suffix);
    world
        .smart_contract_state()
        .range(prefix.clone()..)
        .next()
        .is_some_and(|(key, _)| key.to_string().starts_with(&prefix.to_string()))
}
pub(crate) fn head_key(provider: ProviderId) -> StatePath {
    path(provider, "head")
}
pub(crate) fn record_key(provider: ProviderId, revision: u64) -> StatePath {
    path(provider, &format!("revision_{revision:020}"))
}
pub(crate) fn slot_key(provider: ProviderId, id: [u8; 32]) -> StatePath {
    path(provider, &format!("slot_{}", hex::encode(id)))
}
pub(crate) fn admission_key(provider: ProviderId, id: [u8; 32]) -> StatePath {
    path(provider, &format!("admission_{}", hex::encode(id)))
}

fn audit_after(record: &OperationRecordV1) -> SignerOperationAuditHeadV1 {
    match record.operation.operation.outcome {
        StreamTokenOutcomeV1::Completed(value) => value.commitment.audit,
        StreamTokenOutcomeV1::Reserved | StreamTokenOutcomeV1::Expired => {
            record.operation.operation.reviewed.intent.previous_audit
        }
    }
}

/// Read one bounded immutable revision, checking its local structure and provider scope.
pub(crate) fn read_record(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    revision: u64,
) -> Result<OperationRecordV1, Error> {
    if revision == 0 || revision > MAX_REVISIONS {
        return Err(Error::CorruptHistory);
    }
    let bytes = world
        .smart_contract_state()
        .get(&record_key(provider, revision))
        .ok_or(Error::CorruptHistory)?;
    let record: OperationRecordV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
    let row = &record.operation;
    if record.revision != revision
        || record.request_digest == [0; 32]
        || (revision == 1) != (record.predecessor_digest == [0; 32])
        || validate_stream_token_native_operation_claim_v1(
            row,
            provider,
            row.custody_control_revision,
            row.custody_control_digest,
            &row.reserved_execution.authority,
        )
        .is_err()
    {
        return Err(Error::CorruptHistory);
    }
    Ok(record)
}

/// Read the latest head and adjacent immutable rows without accepting a rolled-back head.
pub(crate) fn read_head(
    world: &impl WorldReadOnly,
    provider: ProviderId,
) -> Result<OperationHeadV1, Error> {
    let Some(bytes) = world.smart_contract_state().get(&head_key(provider)) else {
        if ["revision_", "slot_", "admission_"]
            .into_iter()
            .any(|suffix| prefix_has_any(world, provider, suffix))
        {
            return Err(Error::CorruptHistory);
        }
        return Ok(OperationHeadV1::empty());
    };
    let head: OperationHeadV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
    if head.revision == 0
        || head.revision > MAX_REVISIONS
        || head.digest == [0; 32]
        || head.fence == 0
        || head.fence != head.total_admissions
        || head.total_admissions > STREAM_TOKEN_NATIVE_MAX_OPERATIONS_V1
        || head.audit.sequence > head.total_admissions
        || (head.audit.sequence == 0) != (head.audit.digest == [0; 32])
    {
        return Err(Error::CorruptHistory);
    }
    let current = read_record(world, provider, head.revision)?;
    if record_digest(&current)? != head.digest
        || head.audit != audit_after(&current)
        || head.fence != current.operation.operation.reservation.fence
        || head.active_operation
            != matches!(
                current.operation.operation.outcome,
                StreamTokenOutcomeV1::Reserved
            )
            .then_some(current.operation.operation.reviewed.request.operation_id)
    {
        return Err(Error::CorruptHistory);
    }
    let latest = world
        .smart_contract_state()
        .range(record_key(provider, 0)..=record_key(provider, u64::MAX))
        .next_back();
    if latest.map(|(key, _)| key) != Some(&record_key(provider, head.revision)) {
        return Err(Error::CorruptHistory);
    }
    let id = current.operation.operation.reviewed.request.operation_id;
    let slot: u64 = decode(
        world
            .smart_contract_state()
            .get(&slot_key(provider, id))
            .ok_or(Error::CorruptHistory)?,
    )
    .map_err(|_| Error::CorruptHistory)?;
    if slot != head.revision
        || world
            .smart_contract_state()
            .get(&admission_key(provider, id))
            .is_none()
    {
        return Err(Error::CorruptHistory);
    }
    if head.revision == 1 {
        if current.operation.operation.outcome != StreamTokenOutcomeV1::Reserved
            || current.operation.operation.reservation.fence != 1
        {
            return Err(Error::CorruptHistory);
        }
    } else {
        let previous = read_record(world, provider, head.revision - 1)?;
        if current.predecessor_digest != record_digest(&previous)? {
            return Err(Error::CorruptHistory);
        }
        match current.operation.operation.outcome {
            StreamTokenOutcomeV1::Reserved => {
                if previous.operation.operation.outcome == StreamTokenOutcomeV1::Reserved
                    || current.operation.operation.reservation.fence
                        != previous.operation.operation.reservation.fence + 1
                    || current.operation.operation.reviewed.intent.previous_audit
                        != audit_after(&previous)
                {
                    return Err(Error::CorruptHistory);
                }
            }
            StreamTokenOutcomeV1::Completed(_) | StreamTokenOutcomeV1::Expired => {
                if previous.operation.operation.outcome != StreamTokenOutcomeV1::Reserved
                    || current.operation.operation.reviewed != previous.operation.operation.reviewed
                    || current.operation.operation.reservation
                        != previous.operation.operation.reservation
                    || current.operation.reserved_execution != previous.operation.reserved_execution
                    || current.operation.custody_control_revision
                        != previous.operation.custody_control_revision
                    || current.operation.custody_control_digest
                        != previous.operation.custody_control_digest
                {
                    return Err(Error::CorruptHistory);
                }
            }
        }
    }
    Ok(head)
}

/// Read one exact permanent operation ID and its original/current row pair.
pub(crate) fn read_history(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    id: [u8; 32],
) -> Result<Option<OperationHistoryV1>, Error> {
    let head = read_head(world, provider)?;
    let Some(bytes) = world.smart_contract_state().get(&slot_key(provider, id)) else {
        if world
            .smart_contract_state()
            .get(&admission_key(provider, id))
            .is_some()
        {
            return Err(Error::CorruptHistory);
        }
        return Ok(None);
    };
    let revision: u64 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
    if revision == 0 || revision > head.revision {
        return Err(Error::CorruptHistory);
    }
    let current = read_record(world, provider, revision)?;
    let admitted: u64 = decode(
        world
            .smart_contract_state()
            .get(&admission_key(provider, id))
            .ok_or(Error::CorruptHistory)?,
    )
    .map_err(|_| Error::CorruptHistory)?;
    let original = read_record(world, provider, admitted)?;
    if current.operation.operation.reviewed.request.operation_id != id
        || original.operation.operation.reviewed.request.operation_id != id
        || original.operation.operation.outcome != StreamTokenOutcomeV1::Reserved
        || admitted > revision
        || (revision != admitted
            && (admitted.checked_add(1) != Some(revision)
                || current.predecessor_digest != record_digest(&original)?
                || current.operation.operation.outcome == StreamTokenOutcomeV1::Reserved))
        || (revision == admitted && current != original)
        || current.operation.operation.reviewed != original.operation.operation.reviewed
        || current.operation.operation.reservation != original.operation.operation.reservation
        || current.operation.custody_control_revision != original.operation.custody_control_revision
        || current.operation.custody_control_digest != original.operation.custody_control_digest
        || current.operation.reserved_execution != original.operation.reserved_execution
    {
        return Err(Error::CorruptHistory);
    }
    Ok(Some(OperationHistoryV1 {
        reserved: original,
        current,
    }))
}

/// Read one exact permanent operation ID while preserving the existing latest-row interface.
pub(crate) fn read_slot(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    id: [u8; 32],
) -> Result<Option<OperationRecordV1>, Error> {
    read_history(world, provider, id).map(|history| history.map(|history| history.current))
}

mod historical_execution;
pub use historical_execution::{
    STREAM_TOKEN_HISTORY_FINALITY_MAX_BYTES_V1, STREAM_TOKEN_HISTORY_MAX_BLOCKS_V1,
    VerifiedStreamTokenHistoryV1, authenticate_stream_token_history_to_floor_v1,
};

#[cfg(test)]
#[path = "stream_token_authority/tests.rs"]
mod tests;
