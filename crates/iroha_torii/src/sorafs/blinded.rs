//! Helpers for resolving SoraFS manifests by their blinded CID representations.
//!
//! The gateway ingests daily salt announcements published by the SoraNet Salt
//! Council. Requests can then reference manifests using a BLAKE3 digest derived
//! from the salt and canonical CID instead of exposing the raw identifier on
//! the wire.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use dashmap::DashMap;
use hex::FromHex;
use iroha_crypto::soranet::blinding::canonical_cache_key;
use norito::json::Value as JsonValue;
use sorafs_node::store::StoredManifest;
use thiserror::Error;

/// Width of the blinded CID digest (BLAKE3-256).
pub const BLINDED_CID_LEN: usize = 32;

/// Salt schedule loaded from Norito JSON announcements on disk.
#[derive(Debug)]
pub struct SaltSchedule {
    salts: DashMap<u32, [u8; 32]>,
}

impl SaltSchedule {
    /// Load salt announcements from the supplied directory.
    ///
    /// Each file must be a Norito JSON document containing `epoch_id` and
    /// `blinded_cid_salt_hex` fields, matching the `SaltAnnouncementV1` schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read, a fixture cannot be
    /// parsed, or the salt does not decode to 32 bytes.
    pub fn load_from_dir(dir: &Path) -> Result<Self, SaltScheduleError> {
        let schedule = SaltSchedule {
            salts: DashMap::new(),
        };

        if !dir.exists() {
            return Err(SaltScheduleError::DirectoryMissing(dir.to_path_buf()));
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                if !ext.eq_ignore_ascii_case("json") && !ext.eq_ignore_ascii_case("norito.json") {
                    continue;
                }
            }

            let contents = fs::read_to_string(&path)?;
            let value: norito::json::Value =
                norito::json::from_str(&contents).map_err(|source| SaltScheduleError::Parse {
                    path: path.clone(),
                    source,
                })?;

            let epoch = value
                .get("epoch_id")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| SaltScheduleError::MissingField {
                    path: path.clone(),
                    field: "epoch_id",
                })?;
            let epoch_u32 =
                u32::try_from(epoch).map_err(|_| SaltScheduleError::EpochOutOfRange {
                    path: path.clone(),
                    epoch,
                })?;

            let salt_hex = value
                .get("blinded_cid_salt_hex")
                .and_then(|value| value.as_str())
                .ok_or_else(|| SaltScheduleError::MissingField {
                    path: path.clone(),
                    field: "blinded_cid_salt_hex",
                })?;

            let mut salt = [0u8; 32];
            let decoded =
                Vec::from_hex(salt_hex.trim()).map_err(|source| SaltScheduleError::SaltDecode {
                    path: path.clone(),
                    source,
                })?;
            if decoded.len() != salt.len() {
                return Err(SaltScheduleError::SaltLength {
                    path: path.clone(),
                    len: decoded.len(),
                });
            }
            salt.copy_from_slice(&decoded);

            if schedule.salts.insert(epoch_u32, salt).is_some() {
                return Err(SaltScheduleError::DuplicateEpoch {
                    path: path.clone(),
                    epoch: epoch_u32,
                });
            }
        }

        if schedule.salts.is_empty() {
            return Err(SaltScheduleError::NoAnnouncementsFound(dir.to_path_buf()));
        }

        Ok(schedule)
    }

    /// Returns the 32-byte salt for the supplied epoch.
    #[must_use]
    pub fn salt(&self, epoch: u32) -> Option<[u8; 32]> {
        self.salts.get(&epoch).map(|entry| *entry)
    }
}

/// Error raised while loading the salt schedule.
#[derive(Debug, Error)]
pub enum SaltScheduleError {
    /// Directory containing the announcements does not exist.
    #[error("salt announcement directory {0} does not exist")]
    DirectoryMissing(PathBuf),
    /// I/O failure while scanning the directory.
    #[error("failed to read salt announcements: {0}")]
    Io(#[from] io::Error),
    /// Failed to parse a Norito announcement.
    #[error("failed to parse salt announcement {path}: {source}")]
    Parse {
        /// Path to the failing file.
        path: PathBuf,
        /// Underlying serialization error.
        source: norito::json::Error,
    },
    /// Announcement is missing a required field.
    #[error("salt announcement {path} is missing field `{field}`")]
    MissingField {
        /// Path to the failing file.
        path: PathBuf,
        /// Name of the missing field.
        field: &'static str,
    },
    /// Epoch identifier is outside the supported `u32` range.
    #[error("salt announcement {path} epoch {epoch} exceeds u32 range")]
    EpochOutOfRange {
        /// Path to the failing file.
        path: PathBuf,
        /// Epoch identifier present in the announcement.
        epoch: u64,
    },
    /// Salt could not be decoded from hex.
    #[error("salt announcement {path} contains invalid salt: {source}")]
    SaltDecode {
        /// Path to the failing file.
        path: PathBuf,
        /// Underlying hex parsing error.
        source: hex::FromHexError,
    },
    /// Salt does not decode to 32 bytes.
    #[error("salt announcement {path} decoded salt length {len} (expected 32 bytes)")]
    SaltLength {
        /// Path to the failing file.
        path: PathBuf,
        /// Actual decoded length.
        len: usize,
    },
    /// Two announcements declared the same epoch.
    #[error("salt announcement {path} duplicates epoch {epoch}")]
    DuplicateEpoch {
        /// Path to the duplicate announcement.
        path: PathBuf,
        /// Epoch value that was duplicated.
        epoch: u32,
    },
    /// No announcement files were discovered in the directory.
    #[error("no salt announcements found in {0}")]
    NoAnnouncementsFound(PathBuf),
}

/// Error raised when resolving a blinded CID.
#[derive(Debug, Error, PartialEq, Eq, Copy, Clone)]
pub enum ResolveError {
    /// Requested salt epoch does not exist in the local schedule.
    #[error("salt epoch {0} is not known on this gateway")]
    UnknownEpoch(u32),
}

/// Resolver that maps blinded CIDs to canonical manifest identifiers.
#[derive(Debug)]
pub struct BlindedCidResolver {
    schedule: Arc<SaltSchedule>,
    cache: DashMap<(u32, [u8; BLINDED_CID_LEN]), String>,
    filters: DashMap<u32, Arc<EpochBloom>>,
}

impl BlindedCidResolver {
    /// Construct a new resolver backed by the supplied schedule.
    #[must_use]
    pub fn new(schedule: Arc<SaltSchedule>) -> Self {
        Self {
            schedule,
            cache: DashMap::new(),
            filters: DashMap::new(),
        }
    }

    /// Resolve a blinded CID into a manifest identifier using the gateway's
    /// stored manifests.
    ///
    /// The resolver caches successful lookups for the lifetime of the process.
    ///
    /// # Errors
    ///
    /// Returns [`ResolveError::UnknownEpoch`] when the requested epoch is not
    /// present in the local schedule.
    pub fn resolve_manifest_id(
        &self,
        manifests: &[StoredManifest],
        epoch: u32,
        blinded: &[u8; BLINDED_CID_LEN],
    ) -> Result<Option<String>, ResolveError> {
        if let Some(entry) = self.cache.get(&(epoch, *blinded)) {
            return Ok(Some(entry.clone()));
        }

        let manifest_count = manifests.len();
        let expected_capacity = manifest_count.max(1);
        let filter = match self.filters.entry(epoch) {
            dashmap::mapref::entry::Entry::Occupied(mut occupied) => {
                if manifest_count > occupied.get().capacity_hint() {
                    let bloom = Arc::new(EpochBloom::new(expected_capacity));
                    occupied.insert(Arc::clone(&bloom));
                    bloom
                } else {
                    Arc::clone(occupied.get())
                }
            }
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                Arc::clone(&vacant.insert(Arc::new(EpochBloom::new(expected_capacity))))
            }
        };

        let observed = filter.observed_items();
        let can_short_circuit = observed > 0 && observed >= manifest_count;
        if can_short_circuit && !filter.probably_contains(blinded) {
            return Ok(None);
        }

        let salt = self
            .schedule
            .salt(epoch)
            .ok_or(ResolveError::UnknownEpoch(epoch))?;

        let mut matched_id: Option<String> = None;
        for manifest in manifests {
            let derived = canonical_cache_key(&salt, manifest.manifest_cid());
            let derived_bytes = derived.as_bytes();
            filter.insert(derived_bytes);
            if derived_bytes == blinded {
                matched_id = Some(manifest.manifest_id().to_string());
            }
        }

        filter.record_population(manifest_count);

        matched_id.map_or(Ok(None), |manifest_id| {
            self.cache.insert((epoch, *blinded), manifest_id.clone());
            Ok(Some(manifest_id))
        })
    }
}

/// Lightweight bloom filter used to short-circuit blinded CID lookups.
#[derive(Debug)]
struct EpochBloom {
    bits: Vec<AtomicU64>,
    mask: u64,
    hash_functions: u8,
    capacity_hint: usize,
    observed_items: AtomicUsize,
}

impl EpochBloom {
    fn new(expected_items: usize) -> Self {
        let expected = expected_items.max(1);
        let mut bits = (expected * 16).next_power_of_two();
        if bits < 1024 {
            bits = 1024;
        }
        let bit_mask = bits as u64 - 1;
        let words = bits / 64;
        Self {
            bits: (0..words).map(|_| AtomicU64::new(0)).collect(),
            mask: bit_mask,
            hash_functions: 6,
            capacity_hint: expected,
            observed_items: AtomicUsize::new(0),
        }
    }

    fn capacity_hint(&self) -> usize {
        self.capacity_hint
    }

    fn observed_items(&self) -> usize {
        self.observed_items.load(Ordering::Relaxed)
    }

    fn record_population(&self, count: usize) {
        self.observed_items.store(count, Ordering::Relaxed);
    }

    fn probably_contains(&self, data: &[u8]) -> bool {
        self.check_bits(data, |word, mask| {
            word.load(Ordering::Relaxed) & mask == mask
        })
    }

    fn insert(&self, data: &[u8]) {
        self.check_bits(data, |word, mask| {
            word.fetch_or(mask, Ordering::Relaxed);
            true
        });
    }

    fn check_bits<F>(&self, data: &[u8], mut op: F) -> bool
    where
        F: FnMut(&AtomicU64, u64) -> bool,
    {
        let digest = blake3::hash(data);
        let bytes = digest.as_bytes();
        // Use double hashing to derive k positions.
        let mut h1 = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let mut h2 = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        if h2 == 0 {
            h2 = 0x9e3779b97f4a7c15;
        }
        for _i in 0..self.hash_functions {
            let idx = h1 & self.mask;
            let word_index = (idx >> 6) as usize;
            let bit = 1u64 << (idx & 63);
            if !op(&self.bits[word_index], bit) {
                return false;
            }
            h1 = h1.wrapping_add(h2);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use sorafs_car::PorMerkleTree;
    use sorafs_node::store::{StoredManifestParts, StoredPorTree};

    use super::*;

    fn manifest_with_cid(cid: &[u8], id_hex: &str) -> StoredManifest {
        StoredManifest::from_parts(StoredManifestParts {
            manifest_id: id_hex.to_string(),
            manifest_cid: cid.to_vec(),
            manifest_digest: [0u8; 32],
            payload_digest: [0u8; 32],
            content_length: 0,
            chunk_profile_handle: "sorafs.test@1.0.0".to_string(),
            stripe_layout: None,
            stored_at_unix_secs: 0,
            retention_epoch: 0,
            chunk_files: Vec::new(),
            por_tree: StoredPorTree::from(&PorMerkleTree::empty()),
            manifest_path: PathBuf::from("/tmp/manifest"),
        })
    }

    #[test]
    fn salt_schedule_loads_announcements() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("epoch-000001.norito.json");
        let payload = r#"{
            "epoch_id": 1,
            "valid_after": 0,
            "valid_until": 0,
            "blinded_cid_salt_hex": "ee4fd5a8a2b4e9c0f1dc1a67c962e8d9b4a1c0ffee112233445566778899aabb"
        }"#;
        fs::write(&path, payload).expect("write");

        let schedule = SaltSchedule::load_from_dir(dir.path()).expect("schedule");
        let salt = schedule.salt(1).expect("epoch");
        assert_eq!(
            hex::encode(salt),
            "ee4fd5a8a2b4e9c0f1dc1a67c962e8d9b4a1c0ffee112233445566778899aabb"
        );
        assert!(schedule.salt(2).is_none());
    }

    #[test]
    fn resolver_matches_manifest_by_blinded_cid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("epoch-000042.norito.json");
        let payload = r#"{
            "epoch_id": 42,
            "valid_after": 0,
            "valid_until": 0,
            "blinded_cid_salt_hex": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        }"#;
        fs::write(&path, payload).expect("write");

        let schedule = Arc::new(SaltSchedule::load_from_dir(dir.path()).expect("schedule"));
        let resolver = BlindedCidResolver::new(schedule);

        let manifests = vec![
            manifest_with_cid(b"cid-alpha", "cafebabe"),
            manifest_with_cid(b"cid-beta", "deadbeef"),
        ];

        let salt = resolver.schedule.salt(42).expect("salt");
        let canonical = canonical_cache_key(&salt, manifests[1].manifest_cid());
        let mut blinded = [0u8; BLINDED_CID_LEN];
        blinded.copy_from_slice(canonical.as_bytes());

        let hit = resolver
            .resolve_manifest_id(&manifests, 42, &blinded)
            .expect("resolution");
        assert_eq!(hit.as_deref(), Some("deadbeef"));

        // Cached lookups should not require manifest iteration.
        let cached = resolver
            .resolve_manifest_id(&[], 42, &blinded)
            .expect("cached resolution");
        assert_eq!(cached.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn resolver_bloom_updates_on_manifest_growth() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("epoch-000100.norito.json");
        let payload = r#"{
            "epoch_id": 100,
            "valid_after": 0,
            "valid_until": 0,
            "blinded_cid_salt_hex": "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff00112233445566778899aabbccddeeff"
        }"#;
        fs::write(&path, payload).expect("write");

        let schedule = Arc::new(SaltSchedule::load_from_dir(dir.path()).expect("schedule"));
        let resolver = BlindedCidResolver::new(schedule);

        let manifests = vec![
            manifest_with_cid(b"cid-alpha", "aaaabbbb"),
            manifest_with_cid(b"cid-beta", "ccccdddd"),
        ];

        // Prime the bloom filter.
        for manifest in &manifests {
            let salt = resolver.schedule.salt(100).expect("salt");
            let canonical = canonical_cache_key(&salt, manifest.manifest_cid());
            let mut blinded = [0u8; BLINDED_CID_LEN];
            blinded.copy_from_slice(canonical.as_bytes());
            resolver
                .resolve_manifest_id(&manifests, 100, &blinded)
                .expect("resolution pass");
        }

        // Add a new manifest and ensure resolver still returns it.
        let new_manifest = manifest_with_cid(b"cid-gamma", "eeeeffff");
        let mut manifests_growth = manifests.clone();
        manifests_growth.push(new_manifest.clone());

        let salt = resolver.schedule.salt(100).expect("salt");
        let canonical_new = canonical_cache_key(&salt, new_manifest.manifest_cid());
        let mut blinded_new = [0u8; BLINDED_CID_LEN];
        blinded_new.copy_from_slice(canonical_new.as_bytes());

        let resolved_manifest = resolver
            .resolve_manifest_id(&manifests_growth, 100, &blinded_new)
            .expect("resolution");
        assert_eq!(resolved_manifest.as_deref(), Some("eeeeffff"));
    }
}
