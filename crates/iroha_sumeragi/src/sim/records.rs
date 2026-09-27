//! The fake key store with its installation log, and the record-store id (spec §7.4 record
//! provenance, §12.2, §13.1).
//!
//! - The **key store** holds the node's keys and the *installation log*: one entry per key
//!   (generated on this node, or imported) and one per `(instance, key)` ever started. It may be
//!   backed up and restored from a snapshot ([`KeyStore::clone`]).
//! - The **record store** holds one safety-record file per `(instance, key)` and the *store id*
//!   file next to them. It is never backed up or restored; it can only be lost or replaced by an
//!   empty one. The world keeps the record files per replica; this module owns the id logic.
//!
//! Every log entry is written with a freshly drawn store id: first the id file, then the entry
//! carrying the same id. At start-up, before any installation event, the driver compares the id
//! file with the newest entry's id; on a mismatch (or a missing id while the log has entries)
//! the log does not describe these record files, and the driver durably marks every key
//! imported. An installation event writes the initial record only for a key generated on this
//! node (or with an operator assertion) and never over an existing record file.

use crate::types::{Hash32, PublicKey};

/// A 128-bit record-store id.
pub type StoreId = u128;

/// One installation-log entry; each carries the store id drawn when it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogEntry {
    /// A key installed on the node: generated here (`true`) or imported (`false`).
    Key {
        /// The key.
        key: PublicKey,
        /// Generated on this node and never exported.
        generated: bool,
        /// Store id of this write.
        id: StoreId,
    },
    /// Instance `instance` was started with `key` on this node.
    Instance {
        /// Instance id.
        instance: Hash32,
        /// The key.
        key: PublicKey,
        /// Store id of this write.
        id: StoreId,
    },
}

impl LogEntry {
    fn id(&self) -> StoreId {
        match self {
            Self::Key { id, .. } | Self::Instance { id, .. } => *id,
        }
    }
}

/// The key store with its installation log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyStore {
    /// Installation log, oldest first.
    pub log: Vec<LogEntry>,
}

impl KeyStore {
    /// Append an entry written with the fresh id `id`: the id file first (`store_id`), then the
    /// entry carrying the same id.
    fn append(&mut self, store_id: &mut Option<StoreId>, entry: LogEntry) {
        *store_id = Some(entry.id());
        self.log.push(entry);
    }

    /// Install a key (generated on this node, or imported from a KMS, a backup or another node).
    pub fn install_key(
        &mut self,
        store_id: &mut Option<StoreId>,
        key: &PublicKey,
        generated: bool,
        id: StoreId,
    ) {
        self.append(
            store_id,
            LogEntry::Key {
                key: key.clone(),
                generated,
                id,
            },
        );
    }

    /// Whether `key` counts as generated on this node (its newest key entry says so).
    pub fn generated(&self, key: &PublicKey) -> bool {
        self.log
            .iter()
            .rev()
            .find_map(|entry| match entry {
                LogEntry::Key {
                    key: k, generated, ..
                } if k == key => Some(*generated),
                _ => None,
            })
            .unwrap_or(false)
    }

    /// Whether instance `instance` was ever started with `key` on this node.
    pub fn started(&self, instance: &Hash32, key: &PublicKey) -> bool {
        self.log.iter().any(|entry| {
            matches!(entry, LogEntry::Instance { instance: i, key: k, .. } if i == instance && k == key)
        })
    }

    /// The start-up check (§7.4 rule 3), before any installation event: if the id next to the
    /// record files differs from the newest entry's id, or one of them is missing while the
    /// log has entries, durably mark every key of the log imported (each entry with a fresh id
    /// from `fresh`). Returns whether it did.
    pub fn check_store_id(
        &mut self,
        store_id: &mut Option<StoreId>,
        mut fresh: impl FnMut() -> StoreId,
    ) -> bool {
        let newest = self.log.last().map(LogEntry::id);
        if cfg!(sumeragi_mutation = "MS33d")
            || self.log.is_empty()
            || (newest.is_some() && newest == *store_id)
        {
            return false;
        }
        let mut keys: Vec<PublicKey> = Vec::new();
        for entry in &self.log {
            let (LogEntry::Key { key, .. } | LogEntry::Instance { key, .. }) = entry;
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        for key in keys {
            self.install_key(store_id, &key, false, fresh());
        }
        true
    }

    /// The installation event of `(instance, key)` (§7.4 rule 2) when the node starts the
    /// instance with the key and the log has no entry for it: returns whether the driver writes
    /// the initial record `{instance, key, height: g}` first — only for a key generated on this
    /// node or with an operator assertion that it never signed for the instance, and never over
    /// an existing record file — and then appends the `(instance, key)` entry.
    pub fn install_instance(
        &mut self,
        store_id: &mut Option<StoreId>,
        instance: &Hash32,
        key: &PublicKey,
        record_exists: bool,
        asserted_never_signed: bool,
        id: StoreId,
    ) -> bool {
        if self.started(instance, key) {
            return false;
        }
        let initial = !record_exists && (self.generated(key) || asserted_never_signed);
        self.append(
            store_id,
            LogEntry::Instance {
                instance: *instance,
                key: key.clone(),
                id,
            },
        );
        initial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> PublicKey {
        PublicKey::new(vec![byte; 32]).unwrap()
    }

    #[test]
    fn installation_and_store_id() {
        let (i0, i1) = (Hash32([1; 32]), Hash32([2; 32]));
        let k = key(7);
        let mut next: StoreId = 0;
        let mut fresh = || {
            next += 1;
            next
        };
        let mut store_id = None;
        let mut ks = KeyStore::default();
        // A fresh node: nothing to check.
        assert!(!ks.check_store_id(&mut store_id, &mut fresh));
        ks.install_key(&mut store_id, &k, true, fresh());
        assert!(ks.generated(&k));
        let snapshot = ks.clone();
        // Genesis instance: initial record; again: nothing (already started).
        assert!(ks.install_instance(&mut store_id, &i0, &k, false, false, fresh()));
        assert!(!ks.install_instance(&mut store_id, &i0, &k, false, false, fresh()));
        // A later dataspace instance: initial record, but never over an existing file.
        assert!(!ks.install_instance(&mut store_id, &i1, &k, true, false, fresh()));
        assert!(ks.started(&i1, &k));
        // The id matches the newest entry: the log is trusted.
        assert!(!ks.check_store_id(&mut store_id, &mut fresh));
        // The key store is restored from the snapshot: the ids differ → every key imported.
        let mut rolled_back = snapshot.clone();
        let mut id = store_id;
        assert!(rolled_back.check_store_id(&mut id, &mut fresh));
        assert!(!rolled_back.generated(&k));
        assert!(!rolled_back.install_instance(&mut id, &i1, &k, false, false, fresh()));
        // … unless the operator asserts the key never signed there.
        let mut asserted = snapshot.clone();
        let mut id = store_id;
        asserted.check_store_id(&mut id, &mut fresh);
        assert!(asserted.install_instance(&mut id, &i1, &k, false, true, fresh()));
        // A new, empty record store (no id file) with the old key store: imported as well.
        let mut fresh_disk = snapshot;
        let mut id = None;
        assert!(fresh_disk.check_store_id(&mut id, &mut fresh));
        assert!(!fresh_disk.install_instance(&mut id, &i0, &k, false, false, fresh()));
        // After the marking the log is consistent again.
        assert!(!fresh_disk.check_store_id(&mut id, &mut fresh));
    }
}
