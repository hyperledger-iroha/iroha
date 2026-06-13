//! DA pin intent spool helpers.
//!
//! Torii writes `da-pin-intent-*.norito` artefacts alongside DA commitments.
//! These helpers load and sort pin intents deterministically so they can be
//! threaded into WSV/registry wiring without relying on filesystem ordering.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use iroha_data_model::{
    da::{
        pin_intent::{DaPinIntent, DaPinIntentBundle},
        types::StorageTicketId,
    },
    nexus::LaneId,
};
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
    /// Failed to read a directory entry while scanning the spool.
    #[error("failed to read DA pin spool entry in `{path}`: {source}")]
    ReadEntry {
        /// Spool path being scanned.
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
    /// Pin intent filename does not contain the expected lane/epoch/sequence/ticket/fingerprint tuple.
    #[error("malformed DA pin intent filename at {path}")]
    MalformedFilename {
        /// Path that failed.
        path: PathBuf,
    },
    /// Pin intent filename tuple does not match the decoded pin intent body.
    #[error(
        "DA pin intent filename tuple {filename_lane:?}/{filename_epoch}/{filename_sequence}/{filename_ticket:?} mismatches body {intent_lane:?}/{intent_epoch}/{intent_sequence}/{intent_ticket:?} at {path}"
    )]
    FilenameMismatch {
        /// Path that failed.
        path: PathBuf,
        /// Lane identifier parsed from the filename.
        filename_lane: LaneId,
        /// Epoch parsed from the filename.
        filename_epoch: u64,
        /// Sequence parsed from the filename.
        filename_sequence: u64,
        /// Storage ticket parsed from the filename.
        filename_ticket: StorageTicketId,
        /// Lane identifier decoded from the intent body.
        intent_lane: LaneId,
        /// Epoch decoded from the intent body.
        intent_epoch: u64,
        /// Sequence decoded from the intent body.
        intent_sequence: u64,
        /// Storage ticket decoded from the intent body.
        intent_ticket: StorageTicketId,
    },
}

/// Load all DA pin intents from the spool directory.
///
/// Files are filtered by filename (`da-pin-intent-*.norito`), checked against
/// their advertised lane/epoch/sequence/ticket tuple, decoded using Norito,
/// sorted deterministically, and returned as a vector. When the directory is
/// missing or no intents are present, this returns `Ok(None)`.
///
/// # Errors
///
/// Returns a [`DaPinIntentSpoolError`] if the spool directory cannot be read.
/// Matching pin intent files must be readable, decodable, and match their
/// advertised filename tuple.
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
        let entry = entry.map_err(|source| DaPinIntentSpoolError::ReadEntry {
            path: spool_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !is_da_pin_file(&path)? {
            continue;
        }

        let bytes = std::fs::read(&path).map_err(|source| DaPinIntentSpoolError::ReadFile {
            path: path.clone(),
            source,
        })?;
        intents.push(decode_pin_intent(&bytes, &path)?);
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

fn is_da_pin_file(path: &Path) -> Result<bool, DaPinIntentSpoolError> {
    let Some(name) = path.file_name() else {
        return Ok(false);
    };
    if let Some(name) = name.to_str() {
        return Ok(name.starts_with("da-pin-intent-") && name.ends_with(".norito"));
    }
    if non_utf8_artifact_name_matches(name, b"da-pin-intent-", b".norito") {
        return Err(malformed_filename(path));
    }
    Ok(false)
}

#[cfg(unix)]
fn non_utf8_artifact_name_matches(name: &std::ffi::OsStr, prefix: &[u8], suffix: &[u8]) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let bytes = name.as_bytes();
    bytes.starts_with(prefix) && bytes.ends_with(suffix)
}

#[cfg(not(unix))]
fn non_utf8_artifact_name_matches(_name: &std::ffi::OsStr, _prefix: &[u8], _suffix: &[u8]) -> bool {
    false
}

#[derive(Clone, Copy)]
struct PinIntentFileKey {
    lane_id: LaneId,
    epoch: u64,
    sequence: u64,
    storage_ticket: StorageTicketId,
}

fn parse_pin_intent_file_key(path: &Path) -> Result<PinIntentFileKey, DaPinIntentSpoolError> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(malformed_filename(path));
    };
    let Some(rest) = name
        .strip_prefix("da-pin-intent-")
        .and_then(|name| name.strip_suffix(".norito"))
    else {
        return Err(malformed_filename(path));
    };

    let mut fields = rest.split('-');
    let Some(lane_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(epoch_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(sequence_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(ticket_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    let Some(fingerprint_hex) = fields.next() else {
        return Err(malformed_filename(path));
    };
    if fields.next().is_some() {
        return Err(malformed_filename(path));
    }

    let lane_id = parse_fixed_hex_u32(lane_hex, 8, path).map(LaneId::new)?;
    let epoch = parse_fixed_hex_u64(epoch_hex, 16, path)?;
    let sequence = parse_fixed_hex_u64(sequence_hex, 16, path)?;
    let storage_ticket = StorageTicketId::new(parse_fixed_hex_32(ticket_hex, path)?);
    let _ = parse_fixed_hex_32(fingerprint_hex, path)?;

    Ok(PinIntentFileKey {
        lane_id,
        epoch,
        sequence,
        storage_ticket,
    })
}

fn parse_fixed_hex_u32(
    value: &str,
    width: usize,
    path: &Path,
) -> Result<u32, DaPinIntentSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    u32::from_str_radix(value, 16).map_err(|_| malformed_filename(path))
}

fn parse_fixed_hex_u64(
    value: &str,
    width: usize,
    path: &Path,
) -> Result<u64, DaPinIntentSpoolError> {
    if value.len() != width || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    u64::from_str_radix(value, 16).map_err(|_| malformed_filename(path))
}

fn parse_fixed_hex_32(value: &str, path: &Path) -> Result<[u8; 32], DaPinIntentSpoolError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(malformed_filename(path));
    }
    let mut bytes = [0; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| malformed_filename(path))?;
    Ok(bytes)
}

fn malformed_filename(path: &Path) -> DaPinIntentSpoolError {
    DaPinIntentSpoolError::MalformedFilename {
        path: path.to_path_buf(),
    }
}

fn decode_pin_intent(data: &[u8], path: &Path) -> Result<DaPinIntent, DaPinIntentSpoolError> {
    let filename_key = parse_pin_intent_file_key(path)?;
    let intent =
        decode_from_bytes::<DaPinIntent>(data).map_err(|source| DaPinIntentSpoolError::Decode {
            path: path.to_path_buf(),
            source,
        })?;
    if filename_key.lane_id != intent.lane_id
        || filename_key.epoch != intent.epoch
        || filename_key.sequence != intent.sequence
        || filename_key.storage_ticket != intent.storage_ticket
    {
        return Err(DaPinIntentSpoolError::FilenameMismatch {
            path: path.to_path_buf(),
            filename_lane: filename_key.lane_id,
            filename_epoch: filename_key.epoch,
            filename_sequence: filename_key.sequence,
            filename_ticket: filename_key.storage_ticket,
            intent_lane: intent.lane_id,
            intent_epoch: intent.epoch,
            intent_sequence: intent.sequence,
            intent_ticket: intent.storage_ticket,
        });
    }

    Ok(intent)
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

    fn pin_intent_file_name(intent: &DaPinIntent, fingerprint: [u8; 32]) -> String {
        format!(
            "da-pin-intent-{lane:08x}-{epoch:016x}-{sequence:016x}-{ticket}-{fingerprint}.norito",
            lane = intent.lane_id.as_u32(),
            epoch = intent.epoch,
            sequence = intent.sequence,
            ticket = hex::encode(intent.storage_ticket.as_ref()),
            fingerprint = hex::encode(fingerprint)
        )
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

        let file_a = dir.path().join(pin_intent_file_name(&intent_a, [0xaa; 32]));
        let file_b = dir.path().join(pin_intent_file_name(&intent_b, [0xbb; 32]));

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
    fn load_pin_intents_rejects_corrupt_entries() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let bytes = to_bytes(&intent).expect("encode intent");

        let valid_path = dir.path().join(pin_intent_file_name(&intent, [0xcc; 32]));
        let mut corrupt_key = sample_intent(1, 2);
        corrupt_key.storage_ticket = intent.storage_ticket;
        let corrupt_path = dir
            .path()
            .join(pin_intent_file_name(&corrupt_key, [0xdd; 32]));

        std::fs::write(valid_path, bytes).expect("write valid");
        std::fs::write(corrupt_path, b"corrupt").expect("write corrupt");

        assert!(
            matches!(
                load_pin_intents(dir.path()),
                Err(DaPinIntentSpoolError::Decode { .. })
            ),
            "corrupt pin-intent artifacts must reject the whole spool load"
        );
    }

    #[test]
    fn load_pin_intents_rejects_pin_intent_shaped_directory() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let path = dir.path().join(pin_intent_file_name(&intent, [0x7b; 32]));
        std::fs::create_dir(&path).expect("create pin-intent-shaped directory");

        assert!(
            matches!(
                load_pin_intents(dir.path()),
                Err(DaPinIntentSpoolError::ReadFile { path: observed, .. }) if observed == path
            ),
            "pin-intent-shaped non-files must reject the whole spool load"
        );
    }

    #[cfg(unix)]
    #[test]
    fn pin_intent_file_matcher_rejects_non_utf8_pin_intent_shaped_filename() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let path = PathBuf::from(OsString::from_vec(b"da-pin-intent-\xFF.norito".to_vec()));

        let err = is_da_pin_file(&path).expect_err("non-UTF8 shaped artifact rejects");
        match err {
            DaPinIntentSpoolError::MalformedFilename { path: seen } => assert_eq!(seen, path),
            _ => panic!("expected malformed filename for non-UTF8 DA artifact, got {err:?}"),
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn load_pin_intents_rejects_non_utf8_pin_intent_shaped_filename() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(PathBuf::from(OsString::from_vec(
            b"da-pin-intent-\xFF.norito".to_vec(),
        )));
        std::fs::write(&path, b"ignored").expect("write invalid utf8 filename");

        let err = load_pin_intents(dir.path()).expect_err("non-UTF8 DA artifact rejects");
        match err {
            DaPinIntentSpoolError::MalformedFilename { path: seen } => assert_eq!(seen, path),
            _ => panic!("expected malformed filename for non-UTF8 DA artifact, got {err:?}"),
        }
    }

    #[test]
    fn load_pin_intents_rejects_malformed_filenames() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let bytes = to_bytes(&intent).expect("encode intent");
        let malformed_path = dir
            .path()
            .join("da-pin-intent-00000001-0000000000000001-0000000000000001.norito");

        std::fs::write(malformed_path, bytes).expect("write malformed filename intent");

        assert!(
            matches!(
                load_pin_intents(dir.path()),
                Err(DaPinIntentSpoolError::MalformedFilename { .. })
            ),
            "malformed pin-intent filenames must reject the whole spool load"
        );
    }

    #[test]
    fn load_pin_intents_rejects_filename_tuple_mismatches() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let bytes = to_bytes(&intent).expect("encode intent");
        let mut file_key = intent.clone();
        file_key.sequence = 2;
        let mismatch_path = dir.path().join(pin_intent_file_name(&file_key, [0x99; 32]));

        std::fs::write(mismatch_path, bytes).expect("write mismatch intent");

        assert!(
            matches!(
                load_pin_intents(dir.path()),
                Err(DaPinIntentSpoolError::FilenameMismatch { .. })
            ),
            "pin-intent filename/body tuple mismatches must reject the whole spool load"
        );
    }

    #[test]
    fn load_pin_intents_rejects_filename_ticket_mismatches() {
        let dir = tempdir().expect("tempdir");
        let intent = sample_intent(1, 1);
        let bytes = to_bytes(&intent).expect("encode intent");
        let mut file_key = intent.clone();
        file_key.storage_ticket = StorageTicketId::new([0x99; 32]);
        let mismatch_path = dir.path().join(pin_intent_file_name(&file_key, [0x88; 32]));

        std::fs::write(mismatch_path, bytes).expect("write ticket mismatch intent");

        assert!(
            matches!(
                load_pin_intents(dir.path()),
                Err(DaPinIntentSpoolError::FilenameMismatch { .. })
            ),
            "pin-intent filename/body ticket mismatches must reject the whole spool load"
        );
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
