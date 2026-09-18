//! Mandatory service governance/accounting snapshots with exact MV predecessors.
//!
//! These fields have one snapshot owner here because some keys have no JSON key
//! codec. They are not caches: the outer State capture binds these envelopes to
//! the same publication generation as World. Decoding never derives a predecessor
//! from current balances, configuration, or directory contents.

use super::*;
use norito::json::JsonSerialize as _;

/// Required wire fields; every store includes both current and exact undo maps.
#[derive(JsonSerialize, JsonDeserialize)]
pub(super) struct SnapshotServiceState {
    pub(super) capacity_fee_ledger: snapshot_storage::SnapshotStorage,
    pub(super) capacity_disputes: snapshot_storage::SnapshotStorage,
    pub(super) provider_credit_ledger: snapshot_storage::SnapshotStorage,
    pub(super) sorafs_pricing: Cell<PricingScheduleRecord>,
    pub(super) soradns_directory_records: snapshot_storage::SnapshotStorage,
    pub(super) soradns_directory_pending: snapshot_storage::SnapshotStorage,
    pub(super) soradns_directory_history: snapshot_storage::SnapshotStorage,
    pub(super) soradns_directory_prev_of: snapshot_storage::SnapshotStorage,
    pub(super) soradns_directory_revocations: snapshot_storage::SnapshotStorage,
    pub(super) soradns_release_signers: snapshot_storage::SnapshotStorage,
    pub(super) soradns_directory_latest: Cell<Option<DirectoryId>>,
    pub(super) soradns_rotation_policy: Cell<DirectoryRotationPolicyV1>,
    pub(super) soradns_last_publish_ms: Cell<Option<u64>>,
    pub(super) soradns_history_len: Cell<u64>,
}

macro_rules! serialize_stores {
    ($world:expr, $out:expr, $serialize:ident; $($field:ident),* $(,)?) => {$(
        $out.push_str(concat!(",\"", stringify!($field), "\":"));
        snapshot_storage::$serialize(&$world.$field, $out);
    )*};
}
macro_rules! serialize_cells {
    ($world:expr, $out:expr; $($field:ident),* $(,)?) => {$(
        $out.push_str(concat!(",\"", stringify!($field), "\":"));
        $world.$field.json_serialize($out);
    )*};
}
macro_rules! serialize_fields {
    ($world:expr, $out:expr, $serialize:ident) => {{
        serialize_stores!($world, $out, $serialize; capacity_fee_ledger, capacity_disputes, provider_credit_ledger);
        serialize_cells!($world, $out; sorafs_pricing);
        serialize_stores!($world, $out, $serialize; soradns_directory_records, soradns_directory_pending,
            soradns_directory_history, soradns_directory_prev_of,
            soradns_directory_revocations, soradns_release_signers);
        serialize_cells!($world, $out; soradns_directory_latest, soradns_rotation_policy,
            soradns_last_publish_ms, soradns_history_len);
    }};
}

/// Append the required committed envelopes under the outer State capture fence.
pub(crate) fn serialize(world: &World, out: &mut String) {
    serialize_fields!(world, out, serialize);
}

/// Append the exact maps/cells that consuming this World overlay would publish.
pub(crate) fn serialize_block(world: &WorldBlock<'_>, out: &mut String) {
    serialize_fields!(world, out, serialize_block);
}

fn invalid(message: impl Into<String>) -> json::Error {
    json::Error::InvalidField {
        field: "state.service_state".to_owned(),
        message: message.into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_directory_cut(
    records: &impl StorageReadOnly<DirectoryId, ResolverDirectoryRecordV1>,
    pending: &impl StorageReadOnly<DirectoryId, PendingDirectoryDraftV1>,
    history: &impl StorageReadOnly<u64, DirectoryId>,
    previous: &impl StorageReadOnly<DirectoryId, DirectoryId>,
    latest: Option<DirectoryId>,
    policy: &DirectoryRotationPolicyV1,
    last_publish_ms: Option<u64>,
    history_len: u64,
) -> Result<(), json::Error> {
    if policy.min_interval_ms == 0 || policy.max_skew_ms == 0 || policy.council_threshold == 0 {
        return Err(invalid("invalid retained directory rotation policy"));
    }
    let mut prior = None;
    let mut count = 0_u64;
    let mut seen = BTreeSet::new();
    for (index, id) in history.iter() {
        if *index != count || !seen.insert(*id) {
            return Err(invalid("directory history is not contiguous and unique"));
        }
        let record = records
            .get(id)
            .ok_or_else(|| invalid("directory history references a missing record"))?;
        if record.previous_root != prior || previous.get(id).copied() != prior {
            return Err(invalid("directory history and predecessor links disagree"));
        }
        prior = Some(*id);
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid("directory history length overflow"))?;
    }
    if count != history_len || prior != latest || last_publish_ms.is_some() != latest.is_some() {
        return Err(invalid(
            "directory terminal metadata differs from its history",
        ));
    }
    if records.len() != seen.len() || previous.len() != seen.len().saturating_sub(1) {
        return Err(invalid(
            "directory history does not cover its exact records and links",
        ));
    }
    for (id, draft) in pending.iter() {
        // Drafts may legitimately become stale after another directory publishes.
        // Admission rechecks the predecessor/time/signer at actual use; restore
        // only checks immutable coordinates and does not invent renewed authority.
        if records.get(id).is_some()
            || draft.record.directory_json_sha256 != draft.directory_json_sha256
            || draft.record.builder_public_key != draft.builder_public_key
            || draft.record.builder_signature != draft.builder_signature
        {
            return Err(invalid(
                "retained directory draft conflicts with its immutable payload",
            ));
        }
    }
    Ok(())
}

fn supported_record(key: &DirectoryId, record: &ResolverDirectoryRecordV1) -> bool {
    *key == record.root_hash
        && record.record_version == iroha_data_model::soradns::DIRECTORY_RECORD_VERSION_V1
        && record.rad_count > 0
}

impl SnapshotServiceState {
    /// Decode and validate every current/predecessor envelope before publishing any field.
    pub(super) fn restore(self, world: &mut World) -> Result<(), json::Error> {
        let capacity_fee_ledger = self
            .capacity_fee_ledger
            .decode::<ProviderId, CapacityFeeLedgerEntry>("capacity_fee_ledger", |key, value| {
                *key == value.provider_id
            })?;
        let capacity_disputes = self
            .capacity_disputes
            .decode::<CapacityDisputeId, CapacityDisputeRecord>(
                "capacity_disputes",
                |key, value| *key == value.dispute_id,
            )?;
        let provider_credit_ledger = self
            .provider_credit_ledger
            .decode::<ProviderId, ProviderCreditRecord>(
                "provider_credit_ledger",
                |key, value| *key == value.provider_id,
            )?;
        let records = self
            .soradns_directory_records
            .decode::<DirectoryId, ResolverDirectoryRecordV1>(
                "soradns_directory_records",
                supported_record,
            )?;
        let pending = self
            .soradns_directory_pending
            .decode::<DirectoryId, PendingDirectoryDraftV1>(
                "soradns_directory_pending",
                |key, value| supported_record(key, &value.record),
            )?;
        let history = self
            .soradns_directory_history
            .decode::<u64, DirectoryId>("soradns_directory_history", |_, _| true)?;
        let previous = self
            .soradns_directory_prev_of
            .decode::<DirectoryId, DirectoryId>("soradns_directory_prev_of", |key, value| {
                key != value
            })?;
        let revocations = self
            .soradns_directory_revocations
            .decode::<ResolverId, ResolverRevocationRecordV1>(
                "soradns_directory_revocations",
                |key, value| *key == value.resolver_id,
            )?;
        let signers = self
            .soradns_release_signers
            .decode::<PublicKey, ()>("soradns_release_signers", |_, _| true)?;
        self.sorafs_pricing
            .view()
            .get()
            .validate()
            .map_err(|error| invalid(format!("invalid current pricing: {error}")))?;
        if let Some(prior) = self.sorafs_pricing.predecessor_view().get() {
            prior
                .validate()
                .map_err(|error| invalid(format!("invalid predecessor pricing: {error}")))?;
        }
        validate_directory_cut(
            &records.view(),
            &pending.view(),
            &history.view(),
            &previous.view(),
            *self.soradns_directory_latest.view().get(),
            self.soradns_rotation_policy.view().get(),
            *self.soradns_last_publish_ms.view().get(),
            *self.soradns_history_len.view().get(),
        )?;
        // These are newly decoded, privately owned stores. Temporary reverted
        // overlays borrow actual undo and are dropped without consuming it.
        validate_directory_cut(
            &records.block_and_revert(),
            &pending.block_and_revert(),
            &history.block_and_revert(),
            &previous.block_and_revert(),
            *self.soradns_directory_latest.block_and_revert().get(),
            self.soradns_rotation_policy.block_and_revert().get(),
            *self.soradns_last_publish_ms.block_and_revert().get(),
            *self.soradns_history_len.block_and_revert().get(),
        )?;
        world.capacity_fee_ledger = capacity_fee_ledger;
        world.capacity_disputes = capacity_disputes;
        world.provider_credit_ledger = provider_credit_ledger;
        world.sorafs_pricing = self.sorafs_pricing;
        world.soradns_directory_records = records;
        world.soradns_directory_pending = pending;
        world.soradns_directory_history = history;
        world.soradns_directory_prev_of = previous;
        world.soradns_directory_revocations = revocations;
        world.soradns_release_signers = signers;
        world.soradns_directory_latest = self.soradns_directory_latest;
        world.soradns_rotation_policy = self.soradns_rotation_policy;
        world.soradns_last_publish_ms = self.soradns_last_publish_ms;
        world.soradns_history_len = self.soradns_history_len;
        Ok(())
    }
}

#[cfg(test)]
#[path = "snapshot_service_state_tests.rs"]
pub(crate) mod tests;
