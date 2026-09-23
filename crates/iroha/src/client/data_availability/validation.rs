//! Binding of untrusted DA replies to their request and immutable network context.

use iroha_data_model::{
    NetworkId,
    da::{
        commitment::{DaCommitmentKey, DaCommitmentLocation, DaCommitmentProof},
        pin_intent::{DaPinIntent, DaPinIntentProof},
    },
};
use iroha_torii_shared::da::{
    DaCommitmentListRequest, DaCommitmentListResponse, DaCommitmentProofRequest, DaListSnapshot,
    DaPinIntentListRequest, DaPinIntentListResponse, DaPinIntentQueryRequest,
};

type BindingResult = core::result::Result<(), &'static str>;

pub(super) fn commitment_proof(
    query: &DaCommitmentProofRequest,
    proof: &DaCommitmentProof,
) -> BindingResult {
    let record = &proof.commitment;
    if query
        .manifest_hash
        .is_some_and(|hash| hash != record.manifest_hash)
        || query
            .lane_id
            .is_some_and(|lane| lane != record.lane_id.as_u32())
        || query.epoch.is_some_and(|epoch| epoch != record.epoch)
        || query
            .sequence
            .is_some_and(|sequence| sequence != record.sequence)
    {
        return Err("commitment");
    }
    proof_location(proof.location, proof.bundle_len)
}

pub(super) fn pin_proof(
    query: &DaPinIntentQueryRequest,
    proof: &DaPinIntentProof,
    network: &NetworkId,
) -> BindingResult {
    let record = &proof.intent;
    if query
        .manifest_hash
        .is_some_and(|hash| hash != record.manifest_hash)
        || query
            .storage_ticket
            .is_some_and(|ticket| ticket != record.storage_ticket)
        || query
            .alias
            .as_ref()
            .is_some_and(|alias| Some(alias) != record.alias.as_ref())
        || query
            .lane_id
            .is_some_and(|lane| lane != record.lane_id.as_u32())
        || query.epoch.is_some_and(|epoch| epoch != record.epoch)
        || query
            .sequence
            .is_some_and(|sequence| sequence != record.sequence)
    {
        return Err("intent");
    }
    pin_network(record, network)?;
    proof_location(proof.location, proof.bundle_len)
}

fn proof_location(location: DaCommitmentLocation, bundle_len: u32) -> BindingResult {
    if location.block_height == 0 || location.index_in_bundle >= bundle_len {
        return Err("location");
    }
    Ok(())
}

fn pin_network(intent: &DaPinIntent, network: &NetworkId) -> BindingResult {
    if &intent.authorization.network_id != network
        || &intent.pin_scope_authorization.scope.network_id != network
    {
        return Err("network_id");
    }
    Ok(())
}

pub(super) fn commitments(
    query: &DaCommitmentListRequest,
    response: &DaCommitmentListResponse,
) -> BindingResult {
    let limit = query.page_size().map_err(|_| "limit")?;
    if response.commitments.len() > limit {
        return Err("commitments");
    }
    page(
        query.cursor.map(|cursor| (cursor.snapshot, cursor.after)),
        response
            .next_cursor
            .map(|cursor| (cursor.snapshot, cursor.after)),
        response.commitments.iter().map(|item| {
            (
                DaCommitmentKey::from_record(&item.commitment),
                item.location.block_height,
            )
        }),
    )
}

pub(super) fn pin_intents(
    query: &DaPinIntentListRequest,
    response: &DaPinIntentListResponse,
    network: &NetworkId,
) -> BindingResult {
    let limit = query.page_size().map_err(|_| "limit")?;
    if response.intents.len() > limit {
        return Err("intents");
    }
    if response
        .next_cursor
        .is_some_and(|cursor| cursor.validate().is_err())
    {
        return Err("next_cursor");
    }
    for item in &response.intents {
        pin_network(&item.intent, network)?;
    }
    page(
        query
            .cursor
            .map(|cursor| (cursor.snapshot, location_key(cursor.after))),
        response
            .next_cursor
            .map(|cursor| (cursor.snapshot, location_key(cursor.after))),
        response
            .intents
            .iter()
            .map(|item| (location_key(item.location), item.location.block_height)),
    )
}

fn location_key(location: DaCommitmentLocation) -> (u64, u32) {
    (location.block_height, location.index_in_bundle)
}

fn page<K: Ord + Copy>(
    input: Option<(DaListSnapshot, K)>,
    next: Option<(DaListSnapshot, K)>,
    rows: impl Iterator<Item = (K, u64)>,
) -> BindingResult {
    if let Some((snapshot, after)) = next
        && (!snapshot.is_canonical()
            || input.is_some_and(|(original, previous)| original != snapshot || after <= previous))
    {
        return Err("next_cursor");
    }
    let snapshot = input.or(next).map(|(snapshot, _)| snapshot);
    let mut previous = input.map(|(_, key)| key);
    for (key, height) in rows {
        if previous.is_some_and(|previous| key <= previous)
            || next.is_some_and(|(_, after)| key > after)
            || height == 0
            || snapshot.is_some_and(|snapshot| height > snapshot.block_height)
        {
            return Err("page_order");
        }
        previous = Some(key);
    }
    // The server scans raw rows and filters invisible lanes. An empty page can
    // legitimately carry an advancing cursor beyond the last visible record.
    Ok(())
}
