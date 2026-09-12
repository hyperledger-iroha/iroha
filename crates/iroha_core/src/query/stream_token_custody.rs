//! Bounded same-State native StreamToken custody history lookup.
//!
//! A height index selects the terminal control transition without decoding the full history.
//! The selected revision, its adjacent revisions, the active record and their indexes must agree.
//! These reads establish native control association; callers still own durable Kura/QC finality.
use crate::state::{StateReadOnly, WorldReadOnly};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    stream_token_custody::{
        STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1, STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1,
        STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1, StreamTokenCustodyControlRecordV1,
    },
};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use sorafs_manifest::signer::{
    custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
    protocol::SignerPurposeBindingV1,
    stream_token_custody_control::StreamTokenCustodyControlStateV1,
};
use std::str::FromStr;

/// Payload-free admission and retained-state failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamTokenCustodyControlErrorV1 {
    /// Malformed, oversized or noncanonical public control data.
    Invalid,
    /// The requested chain, network, provider or binding differs from native state.
    BindingMismatch,
    /// Missing/inconsistent retained record, index, predecessor or execution provenance.
    CorruptHistory,
    /// The requested block is absent from this exact committed State view.
    HeightUnavailable,
    /// Exact revision/digest CAS failed.
    Conflict,
    /// The finite normal or emergency revision capacity is exhausted.
    Capacity,
    /// A key/policy generation or enrollment sequence would roll back or revive a revoked key.
    Generation,
    /// The existing independent hardware enrollment verifier rejected the signed record.
    Enrollment,
}
impl std::fmt::Display for StreamTokenCustodyControlErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StreamToken custody control rejected: {self:?}")
    }
}
impl std::error::Error for StreamTokenCustodyControlErrorV1 {}
use StreamTokenCustodyControlErrorV1 as Error;

/// Runtime-only native state snapshot. It is neither decodable nor signed-observation evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamTokenCustodyControlSnapshotV1 {
    /// Exact governed control at the requested committed height.
    pub state: StreamTokenCustodyControlStateV1,
    /// Requested block coordinates paired with the selected native control digest.
    pub anchor: SignerCustodyAnchorV1,
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
#[norito_schema(name = "iroha_core::query::stream_token_custody::ControlIndexV1")]
pub(crate) struct ControlIndexV1 {
    pub(crate) revision: u64,
    pub(crate) digest: [u8; 32],
    pub(crate) height: u64,
    pub(crate) ordinal: u32,
}
pub(crate) struct NativeControl {
    pub(crate) record: StreamTokenCustodyControlRecordV1,
    pub(crate) state: StreamTokenCustodyControlStateV1,
    pub(crate) index: ControlIndexV1,
}

pub(crate) fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Result<Vec<u8>, Error> {
    if norito::canonical_frame_len(value).map_err(|_| Error::Invalid)?
        > STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1
    {
        return Err(Error::Invalid);
    }
    norito::encode_canonical(value).map_err(|_| Error::Invalid)
}
pub(crate) fn decode<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1 {
        return Err(Error::Invalid);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            4096,
            STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1,
            8192,
            256 * 1024,
            16,
        ),
    )
    .map_err(|_| Error::Invalid)
}
pub(crate) fn record_digest(record: &StreamTokenCustodyControlRecordV1) -> Result<[u8; 32], Error> {
    let frame = encode(record)?;
    let mut preimage =
        Vec::with_capacity(STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1.len() + frame.len());
    preimage.extend_from_slice(STREAM_TOKEN_CUSTODY_RECORD_DOMAIN_V1);
    preimage.extend_from_slice(&frame);
    Ok(*Hash::new(preimage).as_ref())
}
fn scope(provider: ProviderId) -> String {
    format!(
        "sorafs_stream_token_custody_v1_{}",
        hex::encode(provider.as_bytes())
    )
}
pub(crate) fn head_key(provider: ProviderId) -> StatePath {
    StatePath::from_str(&format!("{}_head", scope(provider)))
        .expect("bounded native custody head path")
}
pub(crate) fn record_key(provider: ProviderId, revision: u64) -> StatePath {
    StatePath::from_str(&format!("{}_revision_{revision:020}", scope(provider)))
        .expect("bounded native custody revision path")
}
pub(crate) fn height_key(provider: ProviderId, height: u64, ordinal: u32) -> StatePath {
    StatePath::from_str(&format!(
        "{}_height_{height:020}_{ordinal:010}",
        scope(provider)
    ))
    .expect("bounded native custody height path")
}
pub(crate) fn key_path(
    provider: ProviderId,
    signer: bool,
    key: &PublicKey,
) -> Result<StatePath, Error> {
    let digest = Hash::new(encode(key)?);
    StatePath::from_str(&format!(
        "sorafs_stream_token_custody_v1_{}_{}_key_{}",
        hex::encode(provider.as_bytes()),
        if signer { "signer" } else { "attester" },
        hex::encode(digest.as_ref())
    ))
    .map_err(|_| Error::Invalid)
}
fn validate_key_indexes(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    record: &StreamTokenCustodyControlRecordV1,
    state: &StreamTokenCustodyControlStateV1,
) -> Result<(), Error> {
    for (signer, key, generation) in [
        (
            true,
            &state.policy.binding.public_key,
            state.policy.binding.key_revision,
        ),
        (
            false,
            &state.policy.attester_public_key,
            state.policy.attester_authority.key_revision,
        ),
    ] {
        let bytes = world
            .smart_contract_state()
            .get(&key_path(provider, signer, key)?)
            .ok_or(Error::CorruptHistory)?;
        let first: ControlIndexV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
        if first.revision == 0 || first.revision > record.revision {
            return Err(Error::CorruptHistory);
        }
        let original_bytes = world
            .smart_contract_state()
            .get(&record_key(provider, first.revision))
            .ok_or(Error::CorruptHistory)?;
        let original: StreamTokenCustodyControlRecordV1 =
            decode(original_bytes).map_err(|_| Error::CorruptHistory)?;
        let original_state: StreamTokenCustodyControlStateV1 =
            decode(&original.control_state).map_err(|_| Error::CorruptHistory)?;
        original_state
            .validate()
            .map_err(|_| Error::CorruptHistory)?;
        let (original_key, original_generation) = if signer {
            (
                &original_state.policy.binding.public_key,
                original_state.policy.binding.key_revision,
            )
        } else {
            (
                &original_state.policy.attester_public_key,
                original_state.policy.attester_authority.key_revision,
            )
        };
        if original.provider_id != provider
            || original.revision != first.revision
            || original.execution_height != first.height
            || original.ordinal != first.ordinal
            || record_digest(&original)? != first.digest
            || original_key != key
            || original_generation != generation
            || !valid_scope(&original_state, provider)
        {
            return Err(Error::CorruptHistory);
        }
    }
    Ok(())
}
fn valid_scope(state: &StreamTokenCustodyControlStateV1, provider: ProviderId) -> bool {
    matches!(state.policy.binding.purpose, SignerPurposeBindingV1::StreamToken { provider_id } if provider_id == *provider.as_bytes())
}
pub(crate) fn read_record(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    revision: u64,
) -> Result<NativeControl, Error> {
    let bytes = world
        .smart_contract_state()
        .get(&record_key(provider, revision))
        .ok_or(Error::CorruptHistory)?;
    let record: StreamTokenCustodyControlRecordV1 =
        decode(bytes).map_err(|_| Error::CorruptHistory)?;
    let state: StreamTokenCustodyControlStateV1 =
        decode(&record.control_state).map_err(|_| Error::CorruptHistory)?;
    state.validate().map_err(|_| Error::CorruptHistory)?;
    if record.provider_id != provider
        || record.revision != revision
        || revision == 0
        || revision > STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1
        || (revision == 1) != (record.predecessor_digest == [0; 32])
        || record.request_digest == [0; 32]
        || state
            .active_head
            .is_some_and(|head| head.approved_anchor.height >= record.execution_height)
        || record.execution_height == 0
        || record.recorded_at_unix_ms == 0
        || record.recorded_at_unix_ms == u64::MAX
        || !valid_scope(&state, provider)
        || encode(&state).map_err(|_| Error::CorruptHistory)? != record.control_state
    {
        return Err(Error::CorruptHistory);
    }
    let index = ControlIndexV1 {
        revision,
        digest: record_digest(&record)?,
        height: record.execution_height,
        ordinal: record.ordinal,
    };
    let index_bytes = world
        .smart_contract_state()
        .get(&height_key(provider, index.height, index.ordinal))
        .ok_or(Error::CorruptHistory)?;
    if decode::<ControlIndexV1>(index_bytes).map_err(|_| Error::CorruptHistory)? != index {
        return Err(Error::CorruptHistory);
    }
    validate_key_indexes(world, provider, &record, &state)?;
    Ok(NativeControl {
        record,
        state,
        index,
    })
}
fn adjacent(previous: &NativeControl, next: &NativeControl) -> Result<(), Error> {
    let expected_ordinal = if previous.index.height == next.index.height {
        previous
            .index
            .ordinal
            .checked_add(1)
            .ok_or(Error::CorruptHistory)?
    } else {
        0
    };
    let old = &previous.state;
    let new = &next.state;
    if new.policy.binding.key_revision < old.policy.binding.key_revision
        || new.policy.binding.policy_revision < old.policy.binding.policy_revision
        || new.policy.attester_authority.key_revision < old.policy.attester_authority.key_revision
        || new.policy.attester_authority.policy_revision
            < old.policy.attester_authority.policy_revision
        || new.next_sequence < old.next_sequence
        || new.next_sequence
            > old
                .next_sequence
                .checked_add(1)
                .ok_or(Error::CorruptHistory)?
        || (old.signer_revoked
            && !new.signer_revoked
            && new.policy.binding.key_revision == old.policy.binding.key_revision)
        || (old.attester_revoked
            && !new.attester_revoked
            && new.policy.attester_authority.key_revision
                == old.policy.attester_authority.key_revision)
    {
        return Err(Error::CorruptHistory);
    }
    if previous.index.revision.checked_add(1) != Some(next.index.revision)
        || next.record.predecessor_digest != previous.index.digest
        || next.index.height < previous.index.height
        || next.index.ordinal != expected_ordinal
        || next.record.recorded_at_unix_ms < previous.record.recorded_at_unix_ms
        || (next.index.height == previous.index.height
            && next.record.recorded_at_unix_ms != previous.record.recorded_at_unix_ms)
    {
        return Err(Error::CorruptHistory);
    }
    Ok(())
}
fn prefix_has_any(world: &impl WorldReadOnly, prefix: &str) -> bool {
    let start = StatePath::from_str(prefix).expect("bounded native custody prefix");
    world
        .smart_contract_state()
        .range(start..)
        .next()
        .is_some_and(|(key, _)| key.to_string().starts_with(prefix))
}
/// Read and validate the active index plus its immediate predecessor without walking history.
pub(crate) fn read_active(
    world: &impl WorldReadOnly,
    provider: ProviderId,
) -> Result<Option<NativeControl>, Error> {
    let Some(bytes) = world.smart_contract_state().get(&head_key(provider)) else {
        if prefix_has_any(world, &format!("{}_revision_", scope(provider)))
            || prefix_has_any(world, &format!("{}_height_", scope(provider)))
        {
            return Err(Error::CorruptHistory);
        }
        return Ok(None);
    };
    let head: ControlIndexV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
    let active = read_record(world, provider, head.revision)?;
    if active.index != head {
        return Err(Error::CorruptHistory);
    }
    if head.revision == 1 {
        if head.ordinal != 0 {
            return Err(Error::CorruptHistory);
        }
    } else {
        adjacent(&read_record(world, provider, head.revision - 1)?, &active)?;
    }
    // No later retained revision/index may be hidden by a rolled-back active pointer.
    let revision_end = record_key(provider, u64::MAX);
    let latest_revision = world
        .smart_contract_state()
        .range(record_key(provider, 0)..=revision_end)
        .next_back();
    let latest_height = world
        .smart_contract_state()
        .range(height_key(provider, 0, 0)..=height_key(provider, u64::MAX, u32::MAX))
        .next_back();
    if latest_revision.map(|(key, _)| key) != Some(&record_key(provider, head.revision))
        || latest_height.map(|(key, _)| key)
            != Some(&height_key(provider, head.height, head.ordinal))
    {
        return Err(Error::CorruptHistory);
    }
    Ok(Some(active))
}
pub(crate) fn read_at(
    world: &impl WorldReadOnly,
    provider: ProviderId,
    height: u64,
) -> Result<Option<NativeControl>, Error> {
    let Some(active) = read_active(world, provider)? else {
        return Ok(None);
    };
    let entry = world
        .smart_contract_state()
        .range(height_key(provider, 0, 0)..=height_key(provider, height, u32::MAX))
        .next_back();
    let Some((key, bytes)) = entry else {
        let first = read_record(world, provider, 1)?;
        if first.index.height <= height {
            return Err(Error::CorruptHistory);
        }
        return Ok(None);
    };
    let index: ControlIndexV1 = decode(bytes).map_err(|_| Error::CorruptHistory)?;
    if key != &height_key(provider, index.height, index.ordinal)
        || index.height > height
        || index.revision > active.index.revision
    {
        return Err(Error::CorruptHistory);
    }
    let active_revision = active.index.revision;
    let selected = if index == active.index {
        active
    } else {
        read_record(world, provider, index.revision)?
    };
    if selected.index != index {
        return Err(Error::CorruptHistory);
    }
    if index.revision > 1 {
        adjacent(
            &read_record(world, provider, index.revision - 1)?,
            &selected,
        )?;
    }
    if index.revision < active_revision {
        let next = read_record(world, provider, index.revision + 1)?;
        adjacent(&selected, &next)?;
        if next.index.height <= height {
            return Err(Error::CorruptHistory);
        }
    }
    Ok(Some(selected))
}
pub(crate) fn validate_state_binding(
    state: &impl StateReadOnly,
    binding: &SignerCustodyBindingV1,
) -> Result<ProviderId, Error> {
    let SignerPurposeBindingV1::StreamToken { provider_id } = binding.purpose else {
        return Err(Error::BindingMismatch);
    };
    if binding.chain_id != state.chain_id().to_string()
        || binding.network_id != *state.network_id().as_bytes()
        || provider_id == [0; 32]
    {
        return Err(Error::BindingMismatch);
    }
    Ok(ProviderId::new(provider_id))
}
/// Reconstruct exact native role control at one committed height in the supplied State view.
///
/// Lookup decodes a constant number of bounded rows using the immutable height index. Before
/// first enrollment, a configured policy with `active_head: None` is a valid approval anchor.
/// This does not replace the caller's durable block and QC verification.
///
/// # Errors
/// Rejects unavailable heights, mismatched chain/network/provider/binding and incoherent history.
pub fn read_stream_token_custody_control_at_v1(
    state: &impl StateReadOnly,
    expected_binding: &SignerCustodyBindingV1,
    height: u64,
) -> Result<Option<StreamTokenCustodyControlSnapshotV1>, Error> {
    let provider = validate_state_binding(state, expected_binding)?;
    let offset = height
        .checked_sub(1)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or(Error::HeightUnavailable)?;
    let block_hash = state
        .block_hashes()
        .get(offset)
        .ok_or(Error::HeightUnavailable)?;
    let Some(selected) = read_at(state.world(), provider, height)? else {
        return Ok(None);
    };
    if selected.state.policy.binding != *expected_binding {
        return Err(Error::BindingMismatch);
    }
    Ok(Some(StreamTokenCustodyControlSnapshotV1 {
        state: selected.state,
        anchor: SignerCustodyAnchorV1 {
            height,
            block_hash: *block_hash.as_ref(),
            state_digest: selected.index.digest,
        },
    }))
}
