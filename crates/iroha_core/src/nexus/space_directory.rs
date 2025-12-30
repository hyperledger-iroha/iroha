//! Lightweight UAID ↔ dataspace bindings maintained by the Space Directory.
//!
//! The canonical UAID capability manifests live in the main Sora Nexus dataspace.
//! Runtime components (portfolio aggregation, allowance enforcement, telemetry)
//! only need a compact view that says which accounts are permitted to act inside
//! which dataspaces. This module provides that mapping; higher-level services
//! are responsible for keeping it in sync with manifest activation/revocation.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    nexus::{AssetPermissionManifest, DataSpaceId, UniversalAccountId},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Deterministic mapping from a UAID to the dataspaces/accounts where it is active.
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct UaidDataspaceBindings {
    entries: BTreeMap<DataSpaceId, BTreeSet<AccountId>>,
}

impl UaidDataspaceBindings {
    /// Returns `true` when the UAID is not associated with any dataspace.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate dataspace bindings (`dataspace_id`, `accounts`).
    pub fn iter(&self) -> impl Iterator<Item = (&DataSpaceId, &BTreeSet<AccountId>)> {
        self.entries.iter()
    }

    /// Returns `true` when the provided account is bound to `dataspace`.
    #[must_use]
    pub fn is_bound_to(&self, dataspace: DataSpaceId, account_id: &AccountId) -> bool {
        self.entries
            .get(&dataspace)
            .is_some_and(|accounts| accounts.contains(account_id))
    }

    /// Returns the dataspace that owns the provided account, if any.
    #[must_use]
    pub fn dataspace_for_account(&self, account_id: &AccountId) -> Option<DataSpaceId> {
        self.entries
            .iter()
            .find(|(_, accounts)| accounts.contains(account_id))
            .map(|(dataspace, _)| *dataspace)
    }

    /// Adds an `(dataspace, account)` binding for the UAID.
    ///
    /// Returns `true` when the account was newly inserted.
    pub fn bind_account(&mut self, dataspace: DataSpaceId, account_id: AccountId) -> bool {
        self.entries
            .entry(dataspace)
            .or_default()
            .insert(account_id)
    }

    /// Removes an `(dataspace, account)` binding for the UAID.
    ///
    /// Returns `true` when the account was present and removed.
    pub fn unbind_account(&mut self, dataspace: DataSpaceId, account_id: &AccountId) -> bool {
        if let Some(accounts) = self.entries.get_mut(&dataspace) {
            let removed = accounts.remove(account_id);
            let empty = accounts.is_empty();
            if empty {
                let _ = accounts;
                self.entries.remove(&dataspace);
            }
            removed
        } else {
            false
        }
    }

    /// Removes _all_ bindings for the provided account, returning the dataspaces cleared.
    pub fn purge_account(&mut self, account_id: &AccountId) -> Vec<DataSpaceId> {
        let mut emptied = Vec::new();
        for (dataspace, accounts) in &mut self.entries {
            accounts.remove(account_id);
            if accounts.is_empty() {
                emptied.push(*dataspace);
            }
        }
        for dataspace in &emptied {
            self.entries.remove(dataspace);
        }
        emptied
    }
}

/// Manifest record tracked by the Space Directory host.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SpaceDirectoryManifestRecord {
    /// Canonical manifest payload (UAID + dataspace scope).
    pub manifest: AssetPermissionManifest,
    /// Hash of the Norito-encoded manifest bytes.
    pub manifest_hash: Hash,
    /// Lifecycle information populated from activation/expiry/revocation events.
    #[norito(default)]
    pub lifecycle: SpaceDirectoryManifestLifecycle,
}

impl SpaceDirectoryManifestRecord {
    /// Construct a new record computing the canonical manifest hash.
    #[must_use]
    pub fn new(manifest: AssetPermissionManifest) -> Self {
        let manifest_hash: Hash = HashOf::new(&manifest).into();
        Self {
            manifest,
            manifest_hash,
            lifecycle: SpaceDirectoryManifestLifecycle::default(),
        }
    }

    /// Dataspace identifier extracted from the manifest.
    #[must_use]
    pub fn dataspace(&self) -> DataSpaceId {
        self.manifest.dataspace
    }

    /// UAID identifier extracted from the manifest.
    #[must_use]
    pub fn uaid(&self) -> UniversalAccountId {
        self.manifest.uaid
    }

    /// Returns `true` when the manifest is currently active (activated and not expired/revoked).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.lifecycle.is_active()
    }
}

/// Lifecycle metadata recorded for a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
pub struct SpaceDirectoryManifestLifecycle {
    /// Epoch (inclusive) when the manifest actually became active, if known.
    #[norito(default)]
    pub activated_epoch: Option<u64>,
    /// Epoch (inclusive) when the manifest expired naturally.
    #[norito(default)]
    pub expired_epoch: Option<u64>,
    /// Revocation metadata (if the manifest was revoked).
    #[norito(default)]
    pub revocation: Option<SpaceDirectoryManifestRevocation>,
}

/// Metadata describing a manifest revocation event.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SpaceDirectoryManifestRevocation {
    /// Epoch when the revocation took effect.
    pub epoch: u64,
    /// Optional textual reason captured by the host.
    #[norito(default)]
    pub reason: Option<String>,
}

impl SpaceDirectoryManifestLifecycle {
    /// Mark the manifest as activated at the provided epoch (clearing expiry/revocation markers).
    pub fn mark_activated(&mut self, epoch: u64) {
        self.activated_epoch = Some(epoch);
        self.expired_epoch = None;
        self.revocation = None;
    }

    /// Record the epoch when the manifest expired.
    pub fn mark_expired(&mut self, epoch: u64) {
        self.expired_epoch = Some(epoch);
    }

    /// Record a revocation event (epoch + optional reason).
    pub fn mark_revoked(&mut self, epoch: u64, reason: Option<String>) {
        self.revocation = Some(SpaceDirectoryManifestRevocation { epoch, reason });
    }

    /// Returns `true` when the manifest has been activated and not expired/revoked.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.activated_epoch.is_some() && self.expired_epoch.is_none() && self.revocation.is_none()
    }
}

/// Deterministic mapping from dataspace id to manifest record for a UAID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SpaceDirectoryManifestSet {
    entries: BTreeMap<DataSpaceId, SpaceDirectoryManifestRecord>,
}

impl SpaceDirectoryManifestSet {
    /// Returns true when no manifests are recorded for the UAID.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Inserts or replaces the manifest associated with `record.manifest.dataspace`.
    pub fn upsert(
        &mut self,
        record: SpaceDirectoryManifestRecord,
    ) -> Option<SpaceDirectoryManifestRecord> {
        self.entries.insert(record.dataspace(), record)
    }

    /// Removes the manifest bound to the provided dataspace.
    pub fn remove(&mut self, dataspace: &DataSpaceId) -> Option<SpaceDirectoryManifestRecord> {
        self.entries.remove(dataspace)
    }

    /// Fetch the manifest bound to the dataspace, if any.
    #[must_use]
    pub fn get(&self, dataspace: &DataSpaceId) -> Option<&SpaceDirectoryManifestRecord> {
        self.entries.get(dataspace)
    }

    /// Iterate manifests keyed by dataspace.
    pub fn iter(&self) -> impl Iterator<Item = (&DataSpaceId, &SpaceDirectoryManifestRecord)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::nexus::ManifestVersion;
    use iroha_test_samples::gen_account_in;

    use super::*;

    fn sample_manifest(dataspace: u32) -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::V1,
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid::manifest")),
            dataspace: DataSpaceId::new(u64::from(dataspace)),
            issued_ms: 123,
            activation_epoch: 7,
            expiry_epoch: None,
            entries: Vec::new(),
        }
    }

    #[test]
    fn record_computes_manifest_hash() {
        let manifest = sample_manifest(11);
        let expected_hash: Hash = HashOf::new(&manifest).into();
        let record = SpaceDirectoryManifestRecord::new(manifest.clone());
        assert_eq!(record.manifest, manifest);
        assert_eq!(record.manifest_hash, expected_hash);
        assert!(record.lifecycle.activated_epoch.is_none());
    }

    #[test]
    fn manifest_set_upsert_replaces_existing_entry() {
        let mut set = SpaceDirectoryManifestSet::default();
        let first = SpaceDirectoryManifestRecord::new(sample_manifest(1));
        set.upsert(first.clone());
        assert!(!set.is_empty());
        assert_eq!(
            set.get(&DataSpaceId::new(1)).unwrap().manifest_hash,
            first.manifest_hash
        );

        let mut manifest = sample_manifest(1);
        manifest.activation_epoch = 999;
        let replacement = SpaceDirectoryManifestRecord::new(manifest.clone());
        let previous = set.upsert(replacement.clone()).unwrap();
        assert_eq!(previous.manifest_hash, first.manifest_hash);
        let stored = set.get(&DataSpaceId::new(1)).unwrap();
        assert_eq!(stored.manifest.entries, manifest.entries);
        assert_eq!(set.iter().count(), 1);
    }

    #[test]
    fn bindings_report_membership_by_dataspace() {
        let mut bindings = UaidDataspaceBindings::default();
        let dataspace = DataSpaceId::new(7);
        let (account_id, _) = gen_account_in("wonderland");

        assert!(!bindings.is_bound_to(dataspace, &account_id));
        bindings.bind_account(dataspace, account_id.clone());
        assert!(bindings.is_bound_to(dataspace, &account_id));
        assert!(!bindings.is_bound_to(DataSpaceId::new(8), &account_id));
    }
}
