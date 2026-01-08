//! DA pin intent spool helpers.
//!
//! Torii writes `da-pin-intent-*.norito` artefacts alongside DA commitments.
//! These helpers load and sort pin intents deterministically so they can be
//! threaded into WSV/registry wiring without relying on filesystem ordering.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use iroha_data_model::da::{
    pin_intent::{DaPinIntent, DaPinIntentBundle},
    types::StorageTicketId,
};
use iroha_logger::warn;
use norito::decode_from_bytes;
use thiserror::Error;

/// Errors encountered while loading DA pin intents from disk.
#[derive(Debug, Error)]
pub enum DaPinIntentSpoolError {
    /// Directory does not exist or cannot be read.
    #[error("failed to read DA pin spool directory `{path}`: {source}")]
    ReadDir {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to read a pin intent file.
    #[error("failed to read DA pin intent `{path}`: {source}")]
    ReadFile {
        /// Path that failed.
        path: PathBuf,
        /// Source error from the filesystem.
        #[source]
        source: std::io::Error,
    },
    /// Failed to decode a pin intent file.
    #[error("failed to decode DA pin intent `{path}`: {source}")]
    Decode {
        /// Path that failed.
        path: PathBuf,
        /// Source decode error.
        #[source]
        source: norito::core::Error,
    },
}

/// Load all DA pin intents from the spool directory.
///
/// Files are filtered by filename (`da-pin-intent-*.norito`), decoded using
/// Norito, sorted deterministically, and returned as a vector. When the
/// directory is missing or no intents are present, this returns `Ok(None)`.
///
/// # Errors
///
/// Returns a [`DaPinIntentSpoolError`] if the spool directory cannot be read.
/// Individual pin intent files that fail to read or decode are skipped with a warning.
pub fn load_pin_intents(
    spool_dir: &Path,
) -> Result<Option<Vec<DaPinIntent>>, DaPinIntentSpoolError> {
    if !spool_dir.exists() {
        return Ok(None);
    }

    let mut intents = Vec::new();
    let dir_entries =
        std::fs::read_dir(spool_dir).map_err(|source| DaPinIntentSpoolError::ReadDir {
            path: spool_dir.to_path_buf(),
            source,
        })?;

    for entry in dir_entries {
        let entry = match entry {
            Ok(value) => value,
            Err(source) => {
                warn!(?source, "failed to read DA pin spool entry");
                continue;
            }
        };
        let path = entry.path();
        if !is_da_pin_file(&path) {
            continue;
        }

        let bytes = match std::fs::read(&path) {
            Ok(buf) => buf,
            Err(source) => {
                warn!(
                    ?source,
                    path = %path.display(),
                    "failed to read DA pin intent file; skipping"
                );
                continue;
            }
        };

        match decode_from_bytes::<DaPinIntent>(&bytes) {
            Ok(intent) => intents.push(intent),
            Err(source) => {
                warn!(
                    ?source,
                    path = %path.display(),
                    "failed to decode DA pin intent file; skipping"
                );
                continue;
            }
        }
    }

    if intents.is_empty() {
        return Ok(None);
    }

    // Deterministic ordering: by lane, epoch, sequence, then storage ticket bytes.
    intents.sort_by(|a, b| {
        (
            a.lane_id.as_u32(),
            a.epoch,
            a.sequence,
            a.storage_ticket.as_ref(),
        )
            .cmp(&(
                b.lane_id.as_u32(),
                b.epoch,
                b.sequence,
                b.storage_ticket.as_ref(),
            ))
    });

    Ok(Some(intents))
}

fn is_da_pin_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("da-pin-intent-") && name.ends_with(".norito"))
}

/// Drop duplicate/invalid pin intents deterministically and surface the reasons.
/// When duplicate keys appear, keep the latest intent in sort order.
#[must_use]
pub fn canonicalize_bundle(
    bundle: DaPinIntentBundle,
) -> (DaPinIntentBundle, Vec<PinIntentDropReason>) {
    let mut drops = Vec::new();
    let version = bundle.version;
    let mut intents = bundle.intents;
    sort_pin_intents(&mut intents);

    let mut by_key = BTreeMap::<PinIntentKey, (DaPinIntent, usize)>::new();
    for (idx, intent) in intents.into_iter().enumerate() {
        if is_zero_manifest(&intent.manifest_hash) {
            drops.push(PinIntentDropReason::ZeroManifest {
                lane: intent.lane_id.as_u32(),
                epoch: intent.epoch,
                sequence: intent.sequence,
            });
            continue;
        }

        let key = PinIntentKey::from(&intent);
        match by_key.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((intent, idx));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let (kept, kept_idx) = entry.get();
                let (dropped_ticket, dropped_manifest) = if idx > *kept_idx {
                    (kept.storage_ticket, kept.manifest_hash)
                } else {
                    (intent.storage_ticket, intent.manifest_hash)
                };
                drops.push(PinIntentDropReason::DuplicateIntent {
                    lane: key.lane,
                    epoch: key.epoch,
                    sequence: key.sequence,
                    storage_ticket: dropped_ticket,
                    replaced_manifest: dropped_manifest,
                });
                if idx > *kept_idx {
                    entry.insert((intent, idx));
                }
            }
        }
    }

    let mut alias_winners = BTreeMap::<String, (usize, PinIntentKey)>::new();
    for (key, (intent, idx)) in &by_key {
        if let Some(alias) = &intent.alias {
            match alias_winners.entry(alias.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((*idx, *key));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let (previous_idx, previous) = *entry.get();
                    let prev_ticket = by_key
                        .get(&previous)
                        .map_or(intent.storage_ticket, |(intent, _)| intent.storage_ticket);
                    if *idx > previous_idx {
                        entry.insert((*idx, *key));
                        drops.push(PinIntentDropReason::AliasSuperseded {
                            alias: alias.clone(),
                            dropped_ticket: prev_ticket,
                            kept_ticket: intent.storage_ticket,
                        });
                    } else {
                        drops.push(PinIntentDropReason::AliasSuperseded {
                            alias: alias.clone(),
                            dropped_ticket: intent.storage_ticket,
                            kept_ticket: prev_ticket,
                        });
                    }
                }
            }
        }
    }

    let intents: Vec<_> = by_key
        .into_iter()
        .filter(|(key, (intent, _))| {
            intent.alias.as_ref().is_none_or(|alias| {
                alias_winners
                    .get(alias)
                    .is_none_or(|(_, winner_key)| winner_key == key)
            })
        })
        .map(|(_, (intent, _))| intent)
        .collect();

    let mut canonical = DaPinIntentBundle::new(intents);
    canonical.version = version;

    (canonical, drops)
}

/// Reasons why a DA pin intent was dropped while canonicalizing a bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinIntentDropReason {
    /// Manifest hash was zeroed out and cannot be trusted.
    ZeroManifest {
        /// Lane identifier.
        lane: u32,
        /// Epoch the intent targets.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
    },
    /// Duplicate `(lane, epoch, sequence)` entry encountered.
    DuplicateIntent {
        /// Lane identifier.
        lane: u32,
        /// Epoch the intent targets.
        epoch: u64,
        /// Sequence number within the epoch.
        sequence: u64,
        /// Storage ticket associated with the dropped intent.
        storage_ticket: StorageTicketId,
        /// Manifest digest carried by the dropped intent.
        replaced_manifest: iroha_data_model::sorafs::pin_registry::ManifestDigest,
    },
    /// Alias observed multiple times; the lexicographically-latest intent wins.
    AliasSuperseded {
        /// Alias that collided.
        alias: String,
        /// Storage ticket dropped in favor of the winner.
        dropped_ticket: StorageTicketId,
        /// Storage ticket retained for the alias.
        kept_ticket: StorageTicketId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PinIntentKey {
    lane: u32,
    epoch: u64,
    sequence: u64,
}

impl From<&DaPinIntent> for PinIntentKey {
    fn from(intent: &DaPinIntent) -> Self {
        Self {
            lane: intent.lane_id.as_u32(),
            epoch: intent.epoch,
            sequence: intent.sequence,
        }
    }
}

fn sort_pin_intents(intents: &mut [DaPinIntent]) {
    intents.sort_by(|a, b| {
        (
            a.lane_id.as_u32(),
            a.epoch,
            a.sequence,
            *a.storage_ticket.as_bytes(),
            *a.manifest_hash.as_bytes(),
            a.alias.as_deref(),
            a.owner.as_ref(),
        )
            .cmp(&(
                b.lane_id.as_u32(),
                b.epoch,
                b.sequence,
                *b.storage_ticket.as_bytes(),
                *b.manifest_hash.as_bytes(),
                b.alias.as_deref(),
                b.owner.as_ref(),
            ))
    });
}

fn is_zero_manifest(digest: &iroha_data_model::sorafs::pin_registry::ManifestDigest) -> bool {
    digest.as_bytes().iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use std::{convert::TryFrom, path::PathBuf};

    use iroha_data_model::{
        da::{
            pin_intent::{DaPinIntent, DaPinIntentBundle},
            types::StorageTicketId,
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };
    use norito::to_bytes;
    use tempfile::tempdir;

    use super::*;

    fn sample_intent(lane: u32, seq: u64) -> DaPinIntent {
        let lane_byte = u8::try_from(lane).expect("lane id fits in byte for test intent");
        let seq_byte = u8::try_from(seq).expect("sequence fits in byte for test intent");
        DaPinIntent {
            lane_id: LaneId::new(lane),
            epoch: 1,
            sequence: seq,
            storage_ticket: StorageTicketId::new([lane_byte; 32]),
            manifest_hash: ManifestDigest::new([seq_byte; 32]),
            alias: Some(format!("alias-{lane}-{seq}")),
            owner: None,
        }
    }

    #[test]
    fn returns_none_for_missing_dir() {
        let missing = PathBuf::from("this-path-should-not-exist-da-pin-spool");
        assert!(load_pin_intents(&missing).unwrap().is_none());
    }

    #[test]
    fn loads_and_sorts_pin_intents() {
        let dir = tempdir().expect("tempdir");
        let intent_a = sample_intent(2, 5);
        let intent_b = sample_intent(1, 1);

        let bytes_a = to_bytes(&intent_a).expect("encode intent a");
        let bytes_b = to_bytes(&intent_b).expect("encode intent b");

        let file_a = dir
            .path()
            .join("da-pin-intent-00000002-0000000000000001-0000000000000005-a.norito");
        let file_b = dir
            .path()
            .join("da-pin-intent-00000001-0000000000000001-0000000000000001-b.norito");

        std::fs::write(file_a, bytes_a).expect("write a");
        std::fs::write(file_b, bytes_b).expect("write b");

        let intents = load_pin_intents(dir.path())
            .expect("load intents")
            .expect("intents present");

        assert_eq!(intents.len(), 2);
        // Sorted by lane then sequence, so intent_b should come first.
        assert_eq!(intents[0].lane_id, LaneId::new(1));
        assert_eq!(intents[0].sequence, 1);
    }

    #[test]
    fn load_pin_intents_skips_corrupt_entries() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let bytes = to_bytes(&intent).expect("encode intent");

        let valid_path = dir
            .path()
            .join("da-pin-intent-00000001-0000000000000001-0000000000000001-ok.norito");
        let corrupt_path = dir
            .path()
            .join("da-pin-intent-00000001-0000000000000001-0000000000000002-bad.norito");

        std::fs::write(valid_path, bytes).expect("write valid");
        std::fs::write(corrupt_path, b"corrupt").expect("write corrupt");

        let intents = load_pin_intents(dir.path())
            .expect("load intents")
            .expect("intents present");

        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0], intent);
    }

    #[test]
    fn canonicalize_drops_zero_manifest() {
        let mut zero = sample_intent(1, 1);
        zero.manifest_hash = ManifestDigest::new([0; 32]);
        let bundle = DaPinIntentBundle::new(vec![zero.clone(), sample_intent(2, 2)]);

        let (canonical, drops) = canonicalize_bundle(bundle);

        assert_eq!(canonical.intents.len(), 1);
        assert!(drops.contains(&PinIntentDropReason::ZeroManifest {
            lane: zero.lane_id.as_u32(),
            epoch: zero.epoch,
            sequence: zero.sequence,
        }));
    }

    #[test]
    fn canonicalize_prefers_latest_alias() {
        let mut first = sample_intent(1, 1);
        first.alias = Some("dup-alias".to_string());
        let mut second = sample_intent(2, 0);
        second.alias = Some("dup-alias".to_string());
        second.manifest_hash = ManifestDigest::new([0x22; 32]);
        let bundle = DaPinIntentBundle::new(vec![first.clone(), second.clone()]);

        let (canonical, drops) = canonicalize_bundle(bundle);

        assert_eq!(canonical.intents.len(), 1);
        assert_eq!(canonical.intents[0].storage_ticket, second.storage_ticket);
        assert!(drops.iter().any(|drop| matches!(
            drop,
            PinIntentDropReason::AliasSuperseded {
                alias,
                dropped_ticket,
                kept_ticket
            } if alias == "dup-alias"
                && *dropped_ticket == first.storage_ticket
                && *kept_ticket == second.storage_ticket
        )));
    }

    #[test]
    fn canonicalize_replaces_duplicate_key_with_latest_manifest() {
        let mut first = sample_intent(3, 4);
        first.manifest_hash = ManifestDigest::new([0x11; 32]);
        let mut second = first.clone();
        second.manifest_hash = ManifestDigest::new([0x22; 32]);
        let bundle = DaPinIntentBundle::new(vec![first.clone(), second.clone()]);

        let (canonical, drops) = canonicalize_bundle(bundle);

        assert_eq!(canonical.intents.len(), 1);
        assert_eq!(canonical.intents[0].manifest_hash, second.manifest_hash);
        assert!(drops.iter().any(|drop| matches!(
            drop,
            PinIntentDropReason::DuplicateIntent {
                lane,
                epoch,
                sequence,
                storage_ticket,
                replaced_manifest
            } if *lane == first.lane_id.as_u32()
                && *epoch == first.epoch
                && *sequence == first.sequence
                && *storage_ticket == first.storage_ticket
                && *replaced_manifest == first.manifest_hash
        )));
    }

    #[test]
    fn canonicalize_replaces_duplicate_sequence_with_new_ticket() {
        let mut first = sample_intent(5, 6);
        first.storage_ticket = StorageTicketId::new([0x10; 32]);
        first.manifest_hash = ManifestDigest::new([0x11; 32]);
        let mut second = first.clone();
        second.storage_ticket = StorageTicketId::new([0x22; 32]);
        second.manifest_hash = ManifestDigest::new([0x33; 32]);

        let bundle = DaPinIntentBundle::new(vec![first.clone(), second.clone()]);
        let (canonical, drops) = canonicalize_bundle(bundle);

        assert_eq!(canonical.intents.len(), 1);
        assert_eq!(canonical.intents[0].storage_ticket, second.storage_ticket);
        assert_eq!(canonical.intents[0].manifest_hash, second.manifest_hash);
        assert!(drops.iter().any(|drop| matches!(
            drop,
            PinIntentDropReason::DuplicateIntent {
                lane,
                epoch,
                sequence,
                storage_ticket,
                replaced_manifest
            } if *lane == first.lane_id.as_u32()
                && *epoch == first.epoch
                && *sequence == first.sequence
                && *storage_ticket == first.storage_ticket
                && *replaced_manifest == first.manifest_hash
        )));
    }
}
