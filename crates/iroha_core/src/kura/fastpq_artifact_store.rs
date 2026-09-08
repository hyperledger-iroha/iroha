//! Bounded, content-addressed FASTPQ artifact bytes, separate from pipeline metadata.
//!
//! Storage never establishes proof validity, source finality, or spend authority.
//! A recovered byte vector must undergo fresh bounded proof verification against
//! independently authenticated expectations before it can be used as evidence.
//! TODO: connect configured limits, authenticated reference publication and
//! retention to the qualified compact producer before enabling production use.

use std::{
    fs::File,
    io::ErrorKind,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;

use super::{BoundProgressNamespace, Error, Kura, KuraInstanceIdentity, Result};

pub(super) const DIRECTORY: &str = "fastpq_artifacts_v1";
const TEMPORARY: &str = "pending.tmp";
const KIND: &str = "FASTPQ artifact";

/// Explicit storage policy supplied by the owner; no unbounded or environment defaults.
#[derive(Clone, Copy, Debug)]
pub struct FastpqArtifactStorageLimits {
    /// Maximum bytes in one artifact, including its complete nominal wrapper.
    pub max_artifact_bytes: NonZeroUsize,
    /// Maximum stable artifact count. One bounded publication temporary is separate.
    pub max_artifacts: NonZeroUsize,
    /// Maximum aggregate logical bytes, including any publication temporary.
    pub max_total_bytes: NonZeroU64,
}

/// Untrusted content reference, with no claim about proof validity or durability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FastpqStoredArtifactReference {
    digest: [u8; 32],
    byte_len: u64,
}

impl FastpqStoredArtifactReference {
    /// Keep advertised bytes verbatim for exact comparison on bounded recovery.
    ///
    /// In particular, this does not normalize an advertised hash through
    /// `Hash::prehashed`, which could alter its encoded marker bit.
    pub const fn from_advertised(digest: [u8; 32], byte_len: u64) -> Self {
        Self { digest, byte_len }
    }

    /// Hash of the exact complete stored bytes, still untrusted on an advertised reference.
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Exact advertised complete artifact length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn for_bytes(bytes: &[u8]) -> Self {
        Self {
            digest: *Hash::new(bytes).as_ref(),
            byte_len: bytes.len() as u64,
        }
    }

    fn file_name(self) -> String {
        format!("{}.norito", hex::encode(self.digest))
    }
}

/// A successful exact-byte write/readback and file/directory durability barrier.
///
/// Only Kura's persistence operation can construct this receipt. It is a point-in-time
/// storage acknowledgement; it is neither proof verification nor protection against
/// subsequent media corruption. It retains its issuing Kura instance identity.
/// Serialized metadata must never recreate this type.
#[derive(Debug)]
#[must_use = "publish a reference only after receiving its durable storage receipt"]
pub struct FastpqDurableArtifactReceipt {
    reference: FastpqStoredArtifactReference,
    issuer: KuraInstanceIdentity,
}

impl FastpqDurableArtifactReceipt {
    /// Extract content metadata. Reading it later requires a fresh bounded read and proof check.
    pub const fn reference(&self) -> FastpqStoredArtifactReference {
        self.reference
    }

    /// Check issuance by this exact live Kura instance, without reattesting stored bytes.
    pub fn is_from(&self, kura: &Kura) -> bool {
        self.issuer.matches(kura)
    }
}

#[derive(Default)]
struct Inventory {
    stable_count: usize,
    total_bytes: u64,
    temporary: bool,
}

fn invalid(path: impl Into<PathBuf>, message: &'static str) -> Error {
    Error::IO(
        std::io::Error::new(ErrorKind::InvalidData, message),
        path.into(),
    )
}

impl Kura {
    /// Persist bounded opaque FASTPQ artifact bytes without replacing any existing content.
    ///
    /// This method does not decode or admit compact proofs. Its caller must first verify
    /// the artifact under the appropriate proof and authentication policy. The receipt
    /// acknowledges only storage; references belong in a separate bounded metadata record.
    ///
    /// # Errors
    /// Fails for invalid limits, size/quota excess, conflicting content, unsafe namespace
    /// entries, unavailable Kura mutation authority, insufficient configured Kura capacity
    /// after existing reservations, or any failed durability barrier.
    pub fn persist_fastpq_artifact(
        &self,
        bytes: &[u8],
        limits: FastpqArtifactStorageLimits,
    ) -> Result<FastpqDurableArtifactReceipt> {
        let directory = self.store_root.join(DIRECTORY);
        Self::validate_fastpq_artifact_limits(limits, &directory)?;
        if bytes.is_empty() || bytes.len() > limits.max_artifact_bytes.get() {
            return Err(invalid(
                directory,
                "FASTPQ artifact is empty or exceeds its byte cap",
            ));
        }
        let reference = FastpqStoredArtifactReference::for_bytes(bytes);
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.durable_mutation_authorized()?;
        // Preserve Kura's capacity-owner lock order: pending canonical bytes
        // require block-store locks before either geometry or sidecar locks.
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        // Dropping without a published cache delta invalidates Kura's total-usage
        // cache, including directory creation and every error/partial-write path.
        let _usage_mutation = self.begin_total_disk_usage_mutation();
        let namespace = self.open_fastpq_artifact_namespace(true)?;
        let mut inventory = self.inventory_fastpq_artifacts(&namespace, limits)?;
        if inventory.temporary {
            self.discard_fastpq_artifact_temporary(&namespace, limits)?;
            inventory = self.inventory_fastpq_artifacts(&namespace, limits)?;
        }
        let path = directory.join(reference.file_name());
        let existing = self.read_bound_regular_file_bytes_locked(
            &namespace,
            &path,
            limits.max_artifact_bytes.get(),
            KIND,
        )?;
        if let Some(existing) = existing {
            if existing != bytes {
                return Err(invalid(
                    path,
                    "FASTPQ content address contains different bytes",
                ));
            }
        } else {
            if inventory.stable_count >= limits.max_artifacts.get()
                || inventory
                    .total_bytes
                    .checked_add(reference.byte_len)
                    .is_none_or(|total| total > limits.max_total_bytes.get())
            {
                return Err(invalid(path, "FASTPQ artifact storage quota exhausted"));
            }
            // This unrelated payload must not consume capacity reserved for
            // pending canonical blocks, terminal outcomes, carrier components
            // or prune recovery. One no-clobber temporary adds the same bytes
            // as the final object; an exact retry adds no physical payload.
            self.validate_configured_autonomous_mutation_disk_peak_locked(
                pending_canonical_bytes,
                reference.byte_len,
                false,
                false,
                &path,
            )?;
            if !self.publish_bound_noclobber_file_locked(
                &namespace,
                &path,
                &directory.join(TEMPORARY),
                bytes,
                KIND,
            )? {
                return Err(invalid(path, "FASTPQ artifact appeared during publication"));
            }
        }
        // Existing bytes might be a previous publication whose directory sync
        // failed. Presence never skips this exact-object sync/readback barrier.
        let (mut file, metadata) = self.open_bound_regular_file_with_exact_bytes_locked(
            &namespace,
            &path,
            bytes,
            limits.max_artifact_bytes.get(),
            KIND,
        )?;
        sync_artifact_file(&file).map_err(|error| Error::IO(error, path.clone()))?;
        self.sync_native_amx_evidence_namespace(&namespace, KIND)?;
        self.verify_bound_open_regular_file_exact_bytes_after_namespace_mutation_locked(
            &namespace,
            &path,
            &mut file,
            &metadata,
            bytes,
            limits.max_artifact_bytes.get(),
            KIND,
        )?;
        self.inventory_fastpq_artifacts(&namespace, limits)?;
        self.validate_fastpq_artifact_root()?;
        Ok(FastpqDurableArtifactReceipt {
            reference,
            issuer: self.instance_identity(),
        })
    }

    /// Recover exact bounded bytes for an untrusted reference, without decoding a proof.
    ///
    /// Recovery never returns a durable receipt or a verified proof. Fresh proof verification
    /// and separately authenticated source/spend expectations remain mandatory.
    ///
    /// # Errors
    /// Fails for invalid limits/references, missing or changed content, unsafe namespace
    /// entries, quota excess, and unavailable canonical storage. Reads create no directories.
    pub fn read_fastpq_artifact(
        &self,
        reference: FastpqStoredArtifactReference,
        limits: FastpqArtifactStorageLimits,
    ) -> Result<Vec<u8>> {
        let directory = self.store_root.join(DIRECTORY);
        Self::validate_fastpq_artifact_limits(limits, &directory)?;
        if reference.byte_len == 0 || reference.byte_len > limits.max_artifact_bytes.get() as u64 {
            return Err(invalid(
                directory,
                "FASTPQ artifact reference exceeds its byte cap",
            ));
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_guard = self.canonical_chain_lock.lock();
        self.durable_mutation_authorized()?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let namespace = self.open_fastpq_artifact_namespace(false)?;
        self.inventory_fastpq_artifacts(&namespace, limits)?;
        let path = directory.join(reference.file_name());
        let bytes = self
            .read_bound_regular_file_bytes_locked(
                &namespace,
                &path,
                usize::try_from(reference.byte_len)?,
                KIND,
            )?
            .ok_or_else(|| invalid(path.clone(), "FASTPQ artifact reference is absent"))?;
        if FastpqStoredArtifactReference::for_bytes(&bytes) != reference {
            return Err(invalid(
                path,
                "FASTPQ artifact content or exact length differs from reference",
            ));
        }
        self.validate_fastpq_artifact_root()?;
        Ok(bytes)
    }

    fn validate_fastpq_artifact_limits(
        limits: FastpqArtifactStorageLimits,
        directory: &Path,
    ) -> Result<()> {
        if limits.max_artifact_bytes.get() as u64 > limits.max_total_bytes.get()
            || limits.max_artifact_bytes.get() == usize::MAX
            || limits.max_artifacts.get() == usize::MAX
        {
            return Err(invalid(
                directory,
                "FASTPQ artifact storage limits are inconsistent",
            ));
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn validate_fastpq_artifact_root(&self) -> Result<()> {
        if !self.bound_storage_directory_unchanged(&self.store_root_directory) {
            return Err(invalid(
                &self.store_root,
                "FASTPQ artifact Kura root identity changed",
            ));
        }
        Ok(())
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn validate_fastpq_artifact_root(&self) -> Result<()> {
        Err(Error::IO(
            std::io::Error::new(
                ErrorKind::Unsupported,
                "FASTPQ artifact storage requires descriptor-bound directory operations",
            ),
            self.store_root.clone(),
        ))
    }

    fn open_fastpq_artifact_namespace(&self, create: bool) -> Result<BoundProgressNamespace> {
        self.validate_fastpq_artifact_root()?;
        #[cfg(all(unix, not(target_os = "espidf")))]
        if create {
            self.open_or_create_bound_storage_child_directory(
                &self.store_root_directory,
                std::ffi::OsStr::new(DIRECTORY),
            )?;
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        let _ = create;
        let directory = self.store_root.join(DIRECTORY);
        self.open_bound_progress_namespace(&directory.join("content"), &directory.join(TEMPORARY))
    }

    fn inventory_fastpq_artifacts(
        &self,
        namespace: &BoundProgressNamespace,
        limits: FastpqArtifactStorageLimits,
    ) -> Result<Inventory> {
        let directory = self.store_root.join(DIRECTORY);
        let before = self.stable_sidecar_directory_metadata(&directory)?;
        let entries =
            std::fs::read_dir(&directory).map_err(|error| Error::IO(error, directory.clone()))?;
        let mut inventory = Inventory::default();
        // A counter, not an unbounded vector/map: stop before metadata work for
        // any entry beyond the configured stable count plus one temporary.
        for (index, entry) in entries.enumerate() {
            if index > limits.max_artifacts.get() {
                return Err(invalid(
                    &directory,
                    "FASTPQ artifact directory entry cap exceeded",
                ));
            }
            let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
            let path = entry.path();
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(invalid(path, "FASTPQ artifact filename is not canonical"));
            };
            let temporary = name == TEMPORARY;
            if !temporary {
                let hash = name.strip_suffix(".norito").unwrap_or_default();
                if hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err(invalid(
                        path,
                        "FASTPQ artifact filename is not a lowercase content hash",
                    ));
                }
                inventory.stable_count += 1;
                if inventory.stable_count > limits.max_artifacts.get() {
                    return Err(invalid(
                        path,
                        "FASTPQ stable artifact count exceeds its cap",
                    ));
                }
            } else {
                inventory.temporary = true;
            }
            let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, &directory)?
                .ok_or_else(|| {
                    invalid(path.clone(), "FASTPQ artifact vanished during inventory")
                })?;
            let size = metadata.file.len();
            if (!temporary && size == 0) || size > limits.max_artifact_bytes.get() as u64 {
                return Err(invalid(
                    path,
                    "FASTPQ artifact inventory has an invalid payload size",
                ));
            }
            inventory.total_bytes = inventory
                .total_bytes
                .checked_add(size)
                .filter(|total| *total <= limits.max_total_bytes.get())
                .ok_or_else(|| {
                    invalid(path, "FASTPQ artifact inventory exceeds aggregate byte cap")
                })?;
        }
        let after = self.stable_sidecar_directory_metadata(&directory)?;
        if !Self::stable_sidecar_directory_metadata_unchanged(&before, &after)
            || !Self::progress_mutation_namespace_unchanged(namespace)
        {
            return Err(invalid(
                directory,
                "FASTPQ artifact directory changed during bounded inventory",
            ));
        }
        self.validate_fastpq_artifact_root()?;
        Ok(inventory)
    }

    fn discard_fastpq_artifact_temporary(
        &self,
        namespace: &BoundProgressNamespace,
        limits: FastpqArtifactStorageLimits,
    ) -> Result<()> {
        let directory = self.store_root.join(DIRECTORY);
        let path = directory.join(TEMPORARY);
        let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, &directory)?
            .ok_or_else(|| invalid(path.clone(), "FASTPQ temporary disappeared before recovery"))?;
        if metadata.file.len() > limits.max_artifact_bytes.get() as u64 {
            return Err(invalid(path, "FASTPQ temporary exceeds its byte cap"));
        }
        let file = Self::open_bound_progress_file(namespace, &path, &metadata)?;
        // A temporary has never earned a reference. Discard only the exact
        // descriptor-bound, single-link object; never promote recovered bytes
        // based solely on their presence or apparent proof structure.
        Self::remove_bound_progress_file_if_matches(namespace, &path, &file, &metadata)
            .map_err(|error| Error::IO(error, path))?;
        self.sync_native_amx_evidence_namespace(namespace, KIND)
    }
}

fn sync_artifact_file(file: &File) -> std::io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_FILE_SYNC.with(|flag| flag.replace(false)) {
        return Err(std::io::Error::other(
            "injected FASTPQ artifact file sync failure",
        ));
    }
    file.sync_all()
}

#[cfg(test)]
std::thread_local! {
    static FAIL_NEXT_FILE_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(all(test, unix, not(target_os = "espidf")))]
mod tests {
    use super::*;

    fn limits(file: usize, count: usize, total: u64) -> FastpqArtifactStorageLimits {
        FastpqArtifactStorageLimits {
            max_artifact_bytes: NonZeroUsize::new(file).unwrap(),
            max_artifacts: NonZeroUsize::new(count).unwrap(),
            max_total_bytes: NonZeroU64::new(total).unwrap(),
        }
    }

    fn generous() -> FastpqArtifactStorageLimits {
        limits(1024, 16, 16 * 1024)
    }

    fn path(kura: &Kura, reference: FastpqStoredArtifactReference) -> PathBuf {
        kura.store_root.join(DIRECTORY).join(reference.file_name())
    }

    #[test]
    fn durable_roundtrip_and_idempotent_retry_keep_one_exact_artifact() {
        let kura = Kura::blank_kura_for_testing();
        let bytes = b"opaque complete nominal proof wrapper";
        let first = kura.persist_fastpq_artifact(bytes, generous()).unwrap();
        let reference = first.reference();
        assert_eq!(reference.byte_len(), bytes.len() as u64);
        assert_eq!(reference.digest(), *Hash::new(bytes).as_ref());
        assert_eq!(
            kura.read_fastpq_artifact(reference, generous()).unwrap(),
            bytes
        );
        let second = kura.persist_fastpq_artifact(bytes, generous()).unwrap();
        assert_eq!(second.reference(), reference);
        assert_eq!(
            std::fs::read_dir(kura.store_root.join(DIRECTORY))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn absent_read_does_not_create_a_storage_directory() {
        let kura = Kura::blank_kura_for_testing();
        let reference = FastpqStoredArtifactReference::for_bytes(b"absent");
        assert!(kura.read_fastpq_artifact(reference, generous()).is_err());
        assert!(!kura.store_root.join(DIRECTORY).exists());
    }

    #[test]
    fn receipt_issuer_is_distinct_even_for_identical_content_in_two_stores() {
        let first = Kura::blank_kura_for_testing();
        let second = Kura::blank_kura_for_testing();
        let left = first.persist_fastpq_artifact(b"abc", generous()).unwrap();
        let right = second.persist_fastpq_artifact(b"abc", generous()).unwrap();
        assert_eq!(left.reference(), right.reference());
        assert!(left.is_from(&first));
        assert!(!left.is_from(&second));
        assert!(right.is_from(&second));
        assert!(!right.is_from(&first));
    }

    #[test]
    fn empty_oversized_and_inconsistent_requests_fail_before_directory_creation() {
        let kura = Kura::blank_kura_for_testing();
        for (bytes, policy) in [
            (&b""[..], generous()),
            (&b"too big"[..], limits(3, 1, 3)),
            (&b"a"[..], limits(8, 1, 4)),
            (&b"a"[..], limits(1, usize::MAX, 1)),
            (&b"a"[..], limits(usize::MAX, 1, u64::MAX)),
        ] {
            assert!(kura.persist_fastpq_artifact(bytes, policy).is_err());
        }
        assert!(!kura.store_root.join(DIRECTORY).exists());
    }

    #[test]
    fn reference_caps_and_exact_lengths_are_checked() {
        let kura = Kura::blank_kura_for_testing();
        let reference = kura
            .persist_fastpq_artifact(b"abc", generous())
            .unwrap()
            .reference();
        for length in [0, 2, 4, 1025, u64::MAX] {
            let changed =
                FastpqStoredArtifactReference::from_advertised(reference.digest(), length);
            assert!(
                kura.read_fastpq_artifact(changed, generous()).is_err(),
                "length {length}"
            );
        }
        assert_eq!(
            kura.read_fastpq_artifact(reference, generous()).unwrap(),
            b"abc"
        );
    }

    #[test]
    fn advertised_hash_is_not_normalized_and_content_is_rehashed() {
        let kura = Kura::blank_kura_for_testing();
        let reference = kura
            .persist_fastpq_artifact(b"abc", generous())
            .unwrap()
            .reference();
        for index in 0..32 {
            let mut digest = reference.digest();
            digest[index] ^= 1;
            let changed = FastpqStoredArtifactReference::from_advertised(digest, 3);
            assert_eq!(changed.digest(), digest);
            // Give the forged address the actual original bytes. Presence and
            // matching length are insufficient, including the hash marker bit.
            std::fs::copy(path(&kura, reference), path(&kura, changed)).unwrap();
            assert!(kura.read_fastpq_artifact(changed, generous()).is_err());
            std::fs::remove_file(path(&kura, changed)).unwrap();
        }
    }

    #[test]
    fn changed_content_is_neither_read_nor_overwritten_on_retry() {
        let kura = Kura::blank_kura_for_testing();
        let reference = kura
            .persist_fastpq_artifact(b"abc", generous())
            .unwrap()
            .reference();
        std::fs::write(path(&kura, reference), b"xyz").unwrap();
        assert!(kura.read_fastpq_artifact(reference, generous()).is_err());
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"xyz");
    }

    #[test]
    fn stable_count_quota_rejects_new_content_but_permits_exact_retry() {
        let kura = Kura::blank_kura_for_testing();
        let policy = limits(8, 1, 16);
        let reference = kura
            .persist_fastpq_artifact(b"abc", policy)
            .unwrap()
            .reference();
        assert!(kura.persist_fastpq_artifact(b"def", policy).is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"def")).exists());
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc", policy)
                .unwrap()
                .reference(),
            reference
        );
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
    }

    #[test]
    fn aggregate_byte_quota_is_reserved_before_writing() {
        let kura = Kura::blank_kura_for_testing();
        let policy = limits(4, 8, 6);
        let _first = kura.persist_fastpq_artifact(b"abc", policy).unwrap();
        let _second = kura.persist_fastpq_artifact(b"def", policy).unwrap();
        assert!(kura.persist_fastpq_artifact(b"g", policy).is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"g")).exists());
        assert_eq!(
            std::fs::read_dir(kura.store_root.join(DIRECTORY))
                .unwrap()
                .count(),
            2
        );
    }

    #[test]
    fn interrupted_empty_partial_and_complete_temporaries_are_discarded_without_publication() {
        for temporary in [&b""[..], &b"part"[..], &b"complete-but-unreferenced"[..]] {
            let kura = Kura::blank_kura_for_testing();
            let _initial = kura
                .persist_fastpq_artifact(b"initial", generous())
                .unwrap();
            let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
            std::fs::write(&temp, temporary).unwrap();
            let result = kura
                .persist_fastpq_artifact(b"fresh-verified-caller-bytes", generous())
                .unwrap();
            assert!(!temp.exists());
            if !temporary.is_empty() {
                assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(temporary)).exists());
            }
            assert_eq!(
                kura.read_fastpq_artifact(result.reference(), generous())
                    .unwrap(),
                b"fresh-verified-caller-bytes"
            );
        }
    }

    #[test]
    fn temporary_bytes_participate_in_recovery_caps() {
        for (temporary, policy) in [
            (&b"12345"[..], limits(4, 2, 8)),
            (&b"1234"[..], limits(4, 2, 6)),
        ] {
            let kura = Kura::blank_kura_for_testing();
            let reference = kura
                .persist_fastpq_artifact(b"abc", policy)
                .unwrap()
                .reference();
            let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
            std::fs::write(&temp, temporary).unwrap();
            assert!(kura.persist_fastpq_artifact(b"x", policy).is_err());
            assert!(kura.read_fastpq_artifact(reference, policy).is_err());
            assert_eq!(std::fs::read(&temp).unwrap(), temporary);
        }
    }

    #[test]
    fn unknown_names_subdirectories_and_too_many_entries_fail_closed() {
        for name in ["unrecognized", "ABC.norito", "../not-used"] {
            let kura = Kura::blank_kura_for_testing();
            let reference = kura
                .persist_fastpq_artifact(b"abc", generous())
                .unwrap()
                .reference();
            let directory = kura.store_root.join(DIRECTORY);
            if name == "../not-used" {
                std::fs::create_dir(directory.join("child")).unwrap();
            } else {
                std::fs::write(directory.join(name), b"x").unwrap();
            }
            assert!(kura.read_fastpq_artifact(reference, generous()).is_err());
            assert!(kura.persist_fastpq_artifact(b"new", generous()).is_err());
        }
        let kura = Kura::blank_kura_for_testing();
        let reference = kura
            .persist_fastpq_artifact(b"abc", generous())
            .unwrap()
            .reference();
        let _second = kura.persist_fastpq_artifact(b"def", generous()).unwrap();
        assert!(
            kura.read_fastpq_artifact(reference, limits(1024, 1, 16384))
                .is_err()
        );
    }

    #[test]
    fn symlink_and_hardlink_artifacts_are_rejected_without_touching_targets() {
        use std::os::unix::fs::symlink;
        for symlink_attack in [true, false] {
            let kura = Kura::blank_kura_for_testing();
            let outside = tempfile::tempdir().unwrap();
            let target = outside.path().join("target");
            std::fs::write(&target, b"abc").unwrap();
            let reference = kura
                .persist_fastpq_artifact(b"abc", generous())
                .unwrap()
                .reference();
            std::fs::remove_file(path(&kura, reference)).unwrap();
            if symlink_attack {
                symlink(&target, path(&kura, reference)).unwrap();
            } else {
                std::fs::hard_link(&target, path(&kura, reference)).unwrap();
            }
            assert!(kura.read_fastpq_artifact(reference, generous()).is_err());
            assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
            assert_eq!(std::fs::read(&target).unwrap(), b"abc");
        }
    }

    #[test]
    fn symlink_directory_and_temporary_are_never_followed_or_removed() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        let kura = Kura::blank_kura_for_testing();
        symlink(outside.path(), kura.store_root.join(DIRECTORY)).unwrap();
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);

        let kura = Kura::blank_kura_for_testing();
        let _initial = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        let target = outside.path().join("target");
        std::fs::write(&target, b"private").unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        symlink(&target, &temp).unwrap();
        assert!(kura.persist_fastpq_artifact(b"def", generous()).is_err());
        assert!(
            std::fs::symlink_metadata(&temp)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"private");
    }

    #[test]
    fn retained_kura_root_identity_rejects_a_replaced_root() {
        let kura = Kura::blank_kura_for_testing();
        let root = kura.store_root.clone();
        let moved = root.with_extension("moved-fastpq-test");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        let result = kura.persist_fastpq_artifact(b"abc", generous());
        let empty = std::fs::read_dir(&root).unwrap().count() == 0;
        std::fs::remove_dir(&root).unwrap();
        std::fs::rename(&moved, &root).unwrap();
        assert!(result.is_err());
        assert!(empty);
    }

    #[test]
    fn existing_file_requires_a_fresh_file_sync_before_every_receipt() {
        let kura = Kura::blank_kura_for_testing();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        for _ in 0..2 {
            FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
            assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
            assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"abc");
        }
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc", generous())
                .unwrap()
                .reference(),
            reference
        );
    }

    #[test]
    fn publication_and_retry_directory_sync_failures_never_mint_receipts() {
        let kura = Kura::blank_kura_for_testing();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        for _ in 0..2 {
            super::super::FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(true));
            assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
            assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"abc");
        }
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc", generous())
                .unwrap()
                .reference(),
            reference
        );
    }

    #[test]
    fn ancestor_sync_failure_is_also_retried_for_existing_content() {
        let kura = Kura::blank_kura_for_testing();
        let reference = kura
            .persist_fastpq_artifact(b"abc", generous())
            .unwrap()
            .reference();
        super::super::fail_progress_sidecar_ancestor_sync_for_tests(0, 1);
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc", generous())
                .unwrap()
                .reference(),
            reference
        );
    }

    #[test]
    fn temporary_removal_sync_failure_stops_fresh_publication() {
        let kura = Kura::blank_kura_for_testing();
        let _initial = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        std::fs::write(&temp, b"partial").unwrap();
        super::super::FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(true));
        assert!(kura.persist_fastpq_artifact(b"def", generous()).is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"def")).exists());
        let receipt = kura.persist_fastpq_artifact(b"def", generous()).unwrap();
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference(), generous())
                .unwrap(),
            b"def"
        );
    }

    #[test]
    fn artifact_mutations_invalidate_the_total_disk_usage_cache() {
        let kura = Kura::blank_kura_for_testing();
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        let enforced_before = kura.refresh_disk_usage_bytes().unwrap();
        let _receipt = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
        assert_eq!(
            kura.refresh_disk_usage_bytes().unwrap(),
            enforced_before + 3
        );
    }

    #[test]
    fn failed_publication_counts_retained_bytes_without_minting_a_receipt() {
        let kura = Kura::blank_kura_for_testing();
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
        let _retry = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
    }

    #[test]
    fn discarded_temporary_bytes_are_removed_from_global_disk_usage() {
        let kura = Kura::blank_kura_for_testing();
        let _initial = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        std::fs::write(&temp, b"partial").unwrap();
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        let _next = kura.persist_fastpq_artifact(b"de", generous()).unwrap();
        assert!(!temp.exists());
        assert_eq!(
            kura.refresh_total_disk_usage_bytes().unwrap(),
            before - 7 + 2
        );
    }

    #[test]
    fn multi_megabyte_artifact_roundtrips_at_exact_capacity_in_separate_store() {
        let kura = Kura::blank_kura_for_testing();
        let bytes = vec![0xA5; 8 * 1024 * 1024 + 37];
        let policy = limits(bytes.len(), 1, bytes.len() as u64);
        let receipt = kura.persist_fastpq_artifact(&bytes, policy).unwrap();
        assert_eq!(receipt.reference().byte_len(), bytes.len() as u64);
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference(), policy)
                .unwrap(),
            bytes
        );
        assert!(kura.persist_fastpq_artifact(b"another", policy).is_err());
        assert_eq!(kura.fastpq_proof_queue_len_for_testing(), 0);
    }

    #[test]
    fn configured_capacity_reserves_prune_headroom_before_new_artifact_bytes() {
        let mut kura = Kura::blank_kura_for_testing();
        let before = kura.refresh_disk_usage_bytes().unwrap();
        assert!(Kura::canonical_prune_intent_maintenance_headroom_bytes() > 0);
        // The bytes fit the local artifact policy and physical free budget,
        // but the configured budget must also retain Kura's recovery reserve.
        std::sync::Arc::get_mut(&mut kura)
            .unwrap()
            .max_disk_usage_bytes = before.checked_add(3).unwrap();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert!(!path(&kura, reference).exists());
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
        assert_eq!(kura.refresh_disk_usage_bytes().unwrap(), before);
    }

    #[test]
    fn configured_capacity_accepts_exact_fit_and_retry_but_rejects_new_content() {
        let mut kura = Kura::blank_kura_for_testing();
        let before = kura.refresh_disk_usage_bytes().unwrap();
        let limit = before
            .checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
            .and_then(|value| value.checked_add(3))
            .unwrap();
        std::sync::Arc::get_mut(&mut kura)
            .unwrap()
            .max_disk_usage_bytes = limit;
        let first = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        let retry = kura.persist_fastpq_artifact(b"abc", generous()).unwrap();
        assert_eq!(first.reference(), retry.reference());
        assert_eq!(kura.refresh_disk_usage_bytes().unwrap(), before + 3);
        assert!(kura.persist_fastpq_artifact(b"d", generous()).is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"d")).exists());
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
        assert_eq!(
            kura.read_fastpq_artifact(first.reference(), generous())
                .unwrap(),
            b"abc"
        );
    }

    #[test]
    fn emergency_startup_cannot_publish_or_restore_artifact_evidence() {
        let kura = Kura::blank_kura_for_testing_in_emergency_fast_mode();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        assert!(kura.persist_fastpq_artifact(b"abc", generous()).is_err());
        assert!(kura.read_fastpq_artifact(reference, generous()).is_err());
        assert!(!kura.store_root.join(DIRECTORY).exists());
    }
}
