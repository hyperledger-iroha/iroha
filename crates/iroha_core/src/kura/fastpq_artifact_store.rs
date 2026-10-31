//! Bounded, content-addressed FASTPQ artifact bytes, separate from pipeline metadata.
//!
//! Storage never establishes proof validity, source finality, or spend authority.
//! A recovered byte vector must undergo fresh bounded proof verification against
//! independently authenticated expectations before it can be used as evidence.
//! `[kura.fastpq_artifacts]` supplies immutable byte/count caps, including the
//! one unpublished temporary; startup and every read/write enforce those caps.
//! Defaults are 1 MiB per complete artifact, 1,024 stable records and 256 MiB
//! aggregate file bytes. Full stores refuse new content; no unreferenced-record
//! inference authorizes deletion. Changing the supplied Config after construction
//! cannot change the live instance policy.
//! Producer records share these exact aggregate count/byte limits and durability barriers.
//! Their ordered positions form a restart cursor only after fresh source/proof verification.
//! TODO: authority-aware retention must preserve every unresolved producer record/artifact.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use iroha_fs::PrivateDurabilityFile as ArtifactDurabilityFile;
#[cfg(not(windows))]
use std::fs::File as ArtifactDurabilityFile;

use iroha_config::parameters::actual::KuraFastpqArtifactPolicy;
use iroha_crypto::Hash;

use super::{BoundProgressNamespace, Error, Kura, KuraInstanceIdentity, Result};

pub(super) const DIRECTORY: &str = "fastpq_artifacts_v1";
pub(super) const TEMPORARY: &str = "pending.tmp";
pub(super) const PRODUCER_SUFFIX: &str = ".producer";
pub(super) const SOURCE_SUFFIX: &str = ".fqs";
const KIND: &str = "FASTPQ artifact";

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

pub(super) fn stable_name(name: &str) -> bool {
    let hash = if let Some(source) = name.strip_suffix(SOURCE_SUFFIX) {
        let Some((height, hash)) = source.split_once('-') else {
            return false;
        };
        if height.len() != 20
            || !height.bytes().all(|c| c.is_ascii_digit())
            || height.parse::<u64>().ok().is_none_or(|height| height == 0)
        {
            return false;
        }
        hash
    } else {
        name.strip_suffix(".norito")
            .or_else(|| name.strip_suffix(PRODUCER_SUFFIX))
            .unwrap_or_default()
    };
    hash.len() == Hash::LENGTH * 2
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(path: impl Into<PathBuf>, message: &'static str) -> Error {
    Error::IO(
        std::io::Error::new(ErrorKind::InvalidData, message),
        path.into(),
    )
}

impl Kura {
    /// Construct the existing isolated test store with an immutable artifact policy.
    #[cfg(test)]
    pub(crate) fn blank_with_fastpq_artifact_policy_for_testing(
        policy: KuraFastpqArtifactPolicy,
    ) -> std::sync::Arc<Self> {
        policy.validate().expect("valid test artifact policy");
        let mut kura = Self::blank_kura_for_testing();
        std::sync::Arc::get_mut(&mut kura)
            .expect("new isolated test store")
            .fastpq_artifact_policy = policy;
        kura
    }

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
    pub fn persist_fastpq_artifact(&self, bytes: &[u8]) -> Result<FastpqDurableArtifactReceipt> {
        self.persist_fastpq_object(
            bytes,
            &FastpqStoredArtifactReference::for_bytes(bytes).file_name(),
        )
    }

    /// Store an immutable source-position record in the same bounded artifact owner.
    /// The key and bytes remain untrusted; the lane joins them to original source and proof.
    pub(crate) fn persist_fastpq_producer_record(
        &self,
        key: Hash,
        bytes: &[u8],
    ) -> Result<FastpqDurableArtifactReceipt> {
        self.persist_fastpq_object(
            bytes,
            &format!("{}{PRODUCER_SUFFIX}", hex::encode(key.as_ref())),
        )
    }

    /// Persist optional exact source transport under the same aggregate quota and barriers.
    /// Native finality and the complete D7 verifier, not this pathname, authenticate it.
    pub(crate) fn persist_fastpq_source_record(
        &self,
        height: u64,
        carrier: iroha_crypto::HashOf<iroha_data_model::block::BlockHeader>,
        bytes: &[u8],
    ) -> Result<FastpqDurableArtifactReceipt> {
        let name = format!(
            "{height:020}-{}{SOURCE_SUFFIX}",
            hex::encode(carrier.as_ref())
        );
        self.persist_fastpq_object(bytes, &name)
    }

    /// Existing aggregate artifact namespace, shared by bounded original source tapes.
    pub(crate) const fn fastpq_source_namespace() -> &'static str {
        DIRECTORY
    }

    /// Same configured complete-object ceiling for source tapes and produced artifacts.
    pub(crate) fn fastpq_object_max_bytes(&self) -> std::num::NonZeroUsize {
        self.fastpq_artifact_policy.max_artifact_bytes
    }

    /// Bind source transport to the original Kura root after its bounded inventory audit.
    /// Release notifications remain owned here; archive consumers never acquire raw fences.
    pub(crate) fn fastpq_source_archive_root(
        &self,
    ) -> Result<crate::query::native_context_archive::ArchiveDirectory> {
        let mut sidecar_releases = self.sidecar_lock.defer_notifications();
        let root = self
            .native_context_archive_root()
            .map_err(|error| Error::IO(error, self.store_root.clone()))?;
        self.validate_fastpq_artifact_inventory_on_startup(&mut sidecar_releases)?;
        Ok(root)
    }

    fn persist_fastpq_object(
        &self,
        bytes: &[u8],
        name: &str,
    ) -> Result<FastpqDurableArtifactReceipt> {
        let limits = self.fastpq_artifact_policy;
        let directory = self.store_root.join(DIRECTORY);
        Self::validate_fastpq_artifact_limits(limits, &directory)?;
        if !stable_name(name) {
            return Err(invalid(
                directory,
                "FASTPQ object name is not a canonical source position",
            ));
        }
        if bytes.is_empty() || bytes.len() > limits.max_artifact_bytes.get() {
            return Err(invalid(
                directory,
                "FASTPQ artifact is empty or exceeds its byte cap",
            ));
        }
        let reference = FastpqStoredArtifactReference::for_bytes(bytes);
        // The complete artifact operation retains each exact physical release
        // source until all sibling fences and resource accounting have retired.
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let (
            mut prune_releases,
            mut canonical_releases,
            mut geometry_releases,
            mut sidecar_releases,
        ) = (
            self.prune_lock.defer_notifications(),
            self.canonical_chain_lock.defer_notifications(),
            self.lane_geometry_lock.defer_notifications(),
            self.sidecar_lock.defer_notifications(),
        );
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _prune_guard = prune_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _prune_guard = self.prune_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(1);
        self.ensure_prune_recovery_not_required()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _canonical_guard = canonical_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _canonical_guard = self.canonical_chain_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(2);
        self.durable_mutation_authorized()?;
        // Preserve Kura's capacity-owner lock order: pending canonical bytes
        // require block-store locks before either geometry or sidecar locks.
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _geometry_guard = geometry_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _geometry_guard = self.lane_geometry_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(3);
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _sidecar_guard = sidecar_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _sidecar_guard = self.sidecar_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(4);
        // Recovery and publication own sequential, disjoint path snapshots.
        // Any error leaves their common resource owner unavailable.
        let mut usage_mutation = self
            .begin_total_disk_usage_mutation()
            .with_resource_children(2);
        let namespace = self.open_fastpq_artifact_namespace(true)?;
        let mut inventory = self.inventory_fastpq_artifacts(&namespace, limits)?;
        if inventory.temporary {
            let temporary_child = usage_mutation.resource_child(vec![directory.join(TEMPORARY)]);
            self.discard_fastpq_artifact_temporary(&namespace, limits)?;
            inventory = self.inventory_fastpq_artifacts(&namespace, limits)?;
            temporary_child.finish();
        } else {
            usage_mutation.resource_batch(0).finish();
        }
        let path = directory.join(name);
        let publication_child =
            usage_mutation.resource_child(vec![path.clone(), directory.join(TEMPORARY)]);
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
                return Err(Error::FastpqArtifactQuota {
                    stable_count: inventory.stable_count,
                    retained_bytes: inventory.total_bytes,
                    incoming_bytes: reference.byte_len,
                    maximum_records: limits.max_artifacts.get(),
                    maximum_bytes: limits.max_total_bytes.get(),
                });
            }
            // This unrelated payload must not consume capacity reserved for
            // pending canonical blocks, terminal outcomes, carrier components
            // or prune recovery. One no-clobber temporary adds the same bytes
            // as the final object; an exact retry adds no physical payload.
            self.validate_publication_disk_peak_locked(
                pending_canonical_bytes,
                reference.byte_len,
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
        #[cfg(not(windows))]
        let _durability_original = {
            let (mut file, metadata) = self.open_bound_regular_file_with_exact_bytes_locked(
                &namespace,
                &path,
                bytes,
                limits.max_artifact_bytes.get(),
                KIND,
            )?;
            sync_artifact_file(&file).map_err(|error| Error::IO(error, path.clone()))?;
            Self::sync_bound_evidence_namespace(&namespace, KIND)?;
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
            file
        };
        #[cfg(windows)]
        let _durability_original =
            self.finish_native_fastpq_object(&namespace, &path, bytes, limits)?;
        self.validate_fastpq_artifact_root()?;
        publication_child.finish();
        usage_mutation.finish_resources_before_disk_rescan();
        Ok(FastpqDurableArtifactReceipt {
            reference,
            issuer: self.instance_identity(),
        })
    }

    #[cfg(windows)]
    fn finish_native_fastpq_object(
        &self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        bytes: &[u8],
        limits: KuraFastpqArtifactPolicy,
    ) -> Result<ArtifactDurabilityFile> {
        use std::io::{Read as _, Seek as _};
        let owner = Self::native_evidence_parent(namespace, path)?;
        let name = path.file_name().expect("native parent validated the leaf");
        let io_error = |error| Error::IO(error, path.to_path_buf());
        // Existing-only native flush access excludes competing writers/deletion. Keep
        // this exact original through the final inventory rather than reopening it.
        let mut original = owner.open_private_durability(name).map_err(io_error)?;
        let before = original.snapshot().map_err(io_error)?;
        let readback = |original: &mut ArtifactDurabilityFile| -> Result<()> {
            original.require_name(owner, name).map_err(io_error)?;
            if before.local_parts().2 != u64::try_from(bytes.len())?
                || bytes.is_empty()
                || bytes.len() > limits.max_artifact_bytes.get()
            {
                return Err(invalid(path, "FASTPQ original has an unexpected extent"));
            }
            original.rewind().map_err(io_error)?;
            let mut observed = Vec::with_capacity(bytes.len());
            original
                .take(u64::try_from(bytes.len())?.saturating_add(1))
                .read_to_end(&mut observed)
                .map_err(io_error)?;
            if observed != bytes {
                return Err(invalid(
                    path,
                    "FASTPQ original differs from exact expected bytes",
                ));
            }
            Ok(())
        };
        let result = (|| {
            readback(&mut original)?;
            sync_artifact_file(&original).map_err(io_error)?;
            Self::sync_bound_evidence_namespace(namespace, KIND)?;
            readback(&mut original)?;
            self.inventory_fastpq_artifacts_with_original(
                namespace,
                limits,
                Some((path, &original)),
            )?;
            Ok(())
        })();
        // A failed read/sync/inventory still closes through original and namespace
        // custody. Preserve the initiating error if either final observation fails.
        let original_check = (|| {
            original.require_name(owner, name).map_err(io_error)?;
            if original.snapshot().map_err(io_error)? != before {
                return Err(invalid(
                    path,
                    "FASTPQ original changed through its durability barrier",
                ));
            }
            Ok(())
        })();
        let namespace_check = Self::native_evidence_parent(namespace, path).map(|_| ());
        result
            .and(original_check)
            .and(namespace_check)
            .map(|()| original)
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
    ) -> Result<Vec<u8>> {
        let limits = self.fastpq_artifact_policy;
        let directory = self.store_root.join(DIRECTORY);
        Self::validate_fastpq_artifact_limits(limits, &directory)?;
        if reference.byte_len == 0 || reference.byte_len > limits.max_artifact_bytes.get() as u64 {
            return Err(invalid(
                directory,
                "FASTPQ artifact reference exceeds its byte cap",
            ));
        }
        // The complete artifact operation retains each exact physical release
        // source until all sibling fences and resource accounting have retired.
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let (mut prune_releases, mut canonical_releases, mut sidecar_releases) = (
            self.prune_lock.defer_notifications(),
            self.canonical_chain_lock.defer_notifications(),
            self.sidecar_lock.defer_notifications(),
        );
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _prune_guard = prune_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _prune_guard = self.prune_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(1);
        self.ensure_prune_recovery_not_required()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _canonical_guard = canonical_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _canonical_guard = self.canonical_chain_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(2);
        self.durable_mutation_authorized()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _sidecar_guard = sidecar_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _sidecar_guard = self.sidecar_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(3);
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

    /// Read one bounded untrusted source-position record without creating an outbox.
    /// `None` is local exact-path absence, never a negative ledger/proof verdict.
    pub(crate) fn read_fastpq_producer_record(&self, key: Hash) -> Result<Option<Vec<u8>>> {
        let directory = self.store_root.join(DIRECTORY);
        let limits = self.fastpq_artifact_policy;
        Self::validate_fastpq_artifact_limits(limits, &directory)?;
        // Producer reads retain the same physical release sources as artifact reads,
        // including exact-path absence and errors after partial fence acquisition.
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let (mut prune_releases, mut canonical_releases, mut sidecar_releases) = (
            self.prune_lock.defer_notifications(),
            self.canonical_chain_lock.defer_notifications(),
            self.sidecar_lock.defer_notifications(),
        );
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _prune_guard = prune_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _prune_guard = self.prune_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(1);
        self.ensure_prune_recovery_not_required()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _canonical_guard = canonical_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _canonical_guard = self.canonical_chain_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(2);
        self.durable_mutation_authorized()?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC358")))]
        let _sidecar_guard = sidecar_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC358"))]
        let _sidecar_guard = self.sidecar_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        original_release_tests::fence_boundary(3);
        self.validate_fastpq_artifact_root()?;
        // The root remains descriptor-bound. Absence does not skip work: the caller
        // still authenticates its exact source and produces/verifies the artifact.
        if self.canonical_sidecar_directory(&directory)?.is_none() {
            self.validate_fastpq_artifact_root()?;
            return Ok(None);
        }
        let namespace = self.open_fastpq_artifact_namespace(false)?;
        self.inventory_fastpq_artifacts(&namespace, limits)?;
        let path = directory.join(format!("{}{PRODUCER_SUFFIX}", hex::encode(key.as_ref())));
        let result = self.read_bound_regular_file_bytes_locked(
            &namespace,
            &path,
            limits.max_artifact_bytes.get(),
            KIND,
        )?;
        self.validate_fastpq_artifact_root()?;
        Ok(result)
    }

    fn validate_fastpq_artifact_limits(
        limits: KuraFastpqArtifactPolicy,
        directory: &Path,
    ) -> Result<()> {
        limits.validate().map_err(|error| {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidInput, error.to_string()),
                directory.to_path_buf(),
            )
        })
    }

    /// Validate existing retained FASTPQ bytes against the immutable configured policy.
    /// This bounded owner scan runs at initialization/re-audit, never at metric scrape.
    pub(super) fn validate_fastpq_artifact_inventory_on_startup(
        &self,
        sidecar_releases: &mut crate::publication_lock::DeferredPublicationFence<'_, ()>,
    ) -> Result<()> {
        let directory = self.store_root.join(DIRECTORY);
        Self::validate_fastpq_artifact_limits(self.fastpq_artifact_policy, &directory)?;
        // The reconciliation caller retains this source through its outer physical
        // guards and final family publication or invalidation.
        #[cfg(not(all(test, sumeragi_core_mutation = "HC361")))]
        let _sidecar_guard = sidecar_releases.lock();
        #[cfg(all(test, sumeragi_core_mutation = "HC361"))]
        let _sidecar_guard = self.sidecar_lock.lock();
        #[cfg(all(test, unix, not(target_os = "espidf")))]
        super::original_inventory_release_tests::fence_boundary(self, 3);
        // Missing storage remains valid even on a platform that cannot publish artifacts.
        // Supported platforms still bind that absence to the live Kura root identity.
        #[cfg(any(windows, all(unix, not(target_os = "espidf"))))]
        self.validate_fastpq_artifact_root()?;
        if self.canonical_sidecar_directory(&directory)?.is_none() {
            return Ok(());
        }
        let namespace = self.open_fastpq_artifact_namespace(false)?;
        self.inventory_fastpq_artifacts(&namespace, self.fastpq_artifact_policy)?;
        Ok(())
    }

    #[cfg(any(windows, all(unix, not(target_os = "espidf"))))]
    fn validate_fastpq_artifact_root(&self) -> Result<()> {
        if !self.bound_storage_directory_unchanged(&self.store_root_directory) {
            return Err(invalid(
                &self.store_root,
                "FASTPQ artifact Kura root identity changed",
            ));
        }
        Ok(())
    }

    #[cfg(not(any(windows, all(unix, not(target_os = "espidf")))))]
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
        #[cfg(any(windows, all(unix, not(target_os = "espidf"))))]
        if create {
            self.open_or_create_bound_storage_child_directory(
                &self.store_root_directory,
                std::ffi::OsStr::new(DIRECTORY),
            )?;
        }
        #[cfg(not(any(windows, all(unix, not(target_os = "espidf")))))]
        let _ = create;
        let directory = self.store_root.join(DIRECTORY);
        self.open_bound_progress_namespace(&directory.join("content"), &directory.join(TEMPORARY))
    }

    fn inventory_fastpq_artifacts(
        &self,
        namespace: &BoundProgressNamespace,
        limits: KuraFastpqArtifactPolicy,
    ) -> Result<Inventory> {
        self.inventory_fastpq_artifacts_with_original(
            namespace,
            limits,
            #[cfg(windows)]
            None,
        )
    }

    fn inventory_fastpq_artifacts_with_original(
        &self,
        namespace: &BoundProgressNamespace,
        limits: KuraFastpqArtifactPolicy,
        #[cfg(windows)] retained: Option<(&Path, &ArtifactDurabilityFile)>,
    ) -> Result<Inventory> {
        #[cfg(windows)]
        if let Some((path, original)) = retained {
            let owner = Self::native_evidence_parent(namespace, path)?;
            original
                .require_name(owner, path.file_name().expect("native leaf"))
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        }
        #[cfg(windows)]
        let mut retained_seen = false;
        let directory = self.store_root.join(DIRECTORY);
        let before = self.stable_sidecar_directory_metadata(&directory)?;
        #[cfg(not(windows))]
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| Error::IO(error, directory.clone()))?
            .enumerate();
        #[cfg(windows)]
        let entries = Self::native_evidence_parent(namespace, &directory.join(TEMPORARY))?
            .entries(limits.max_artifacts.get().saturating_add(1))
            .map_err(|error| Error::IO(error, directory.clone()))?
            .into_iter()
            .enumerate();
        let mut inventory = Inventory::default();
        // A counter, not an unbounded vector/map: stop before metadata work for
        // any entry beyond the configured stable count plus one temporary.
        for (index, entry) in entries {
            if index > limits.max_artifacts.get() {
                return Err(invalid(
                    &directory,
                    "FASTPQ artifact directory entry cap exceeded",
                ));
            }
            #[cfg(not(windows))]
            let (path, name) = {
                let entry = entry.map_err(|error| Error::IO(error, directory.clone()))?;
                (entry.path(), entry.file_name())
            };
            #[cfg(windows)]
            let (path, name) = (directory.join(&entry), entry);
            let Some(name) = name.to_str() else {
                return Err(invalid(path, "FASTPQ artifact filename is not canonical"));
            };
            let temporary = name == TEMPORARY;
            if temporary {
                inventory.temporary = true;
            } else {
                if !stable_name(name) {
                    return Err(invalid(path, "FASTPQ object filename is not canonical"));
                }
                inventory.stable_count += 1;
                if inventory.stable_count > limits.max_artifacts.get() {
                    return Err(invalid(
                        path,
                        "FASTPQ stable artifact count exceeds its cap",
                    ));
                }
            }
            let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, &directory)?
                .ok_or_else(|| {
                    invalid(path.clone(), "FASTPQ artifact vanished during inventory")
                })?;
            #[cfg(windows)]
            {
                let owner = Self::native_evidence_parent(namespace, &path)?;
                if let Some((_, original)) =
                    retained.filter(|(retained_path, _)| *retained_path == path.as_path())
                {
                    original
                        .require_name(owner, name)
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    let before = original
                        .snapshot()
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    if before.local_parts().2 != metadata.file.len() {
                        return Err(invalid(path, "FASTPQ retained inventory extent changed"));
                    }
                    original
                        .revalidate()
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    if original
                        .snapshot()
                        .map_err(|error| Error::IO(error, path.clone()))?
                        != before
                    {
                        return Err(invalid(
                            path,
                            "FASTPQ retained original changed during inventory",
                        ));
                    }
                    retained_seen = true;
                } else {
                    let original = owner
                        .open_retained_regular_optional(name)
                        .map_err(|error| Error::IO(error, path.clone()))?
                        .ok_or_else(|| {
                            invalid(
                                path.clone(),
                                "FASTPQ artifact vanished during private admission",
                            )
                        })?;
                    iroha_fs::FileSnapshot::of(original.file(), true)
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    let opened = super::secure_file_metadata::from_file(original.file())
                        .map_err(|error| Error::IO(error, path.clone()))?;
                    if !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened) {
                        return Err(invalid(
                            path,
                            "FASTPQ artifact changed during private admission",
                        ));
                    }
                    original
                        .revalidate()
                        .map_err(|error| Error::IO(error, path.clone()))?;
                }
            }
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
        #[cfg(windows)]
        if let Some((path, original)) = retained {
            if !retained_seen {
                return Err(invalid(path, "FASTPQ retained inventory entry is missing"));
            }
            let owner = Self::native_evidence_parent(namespace, path)?;
            original
                .require_name(owner, path.file_name().expect("native leaf"))
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
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
        limits: KuraFastpqArtifactPolicy,
    ) -> Result<()> {
        let directory = self.store_root.join(DIRECTORY);
        let path = directory.join(TEMPORARY);
        let metadata = Self::regular_sidecar_metadata_for(&self.store_root, &path, &directory)?
            .ok_or_else(|| invalid(path.clone(), "FASTPQ temporary disappeared before recovery"))?;
        if metadata.file.len() > limits.max_artifact_bytes.get() as u64 {
            return Err(invalid(path, "FASTPQ temporary exceeds its byte cap"));
        }
        // A temporary has never earned a reference. Discard only the exact
        // descriptor-bound, single-link object; never promote recovered bytes
        // based solely on their presence or apparent proof structure.
        #[cfg(not(windows))]
        {
            let file = Self::open_bound_progress_file(namespace, &path, &metadata)?;
            Self::remove_bound_progress_file_if_matches(namespace, &path, &file, &metadata)
                .map_err(|error| Error::IO(error, path.clone()))?;
        }
        #[cfg(windows)]
        {
            let owner = Self::native_evidence_parent(namespace, &path)?;
            let original = owner
                .open_retained_regular_optional(TEMPORARY)
                .map_err(|error| Error::IO(error, path.clone()))?
                .ok_or_else(|| {
                    invalid(
                        path.clone(),
                        "FASTPQ temporary disappeared during admission",
                    )
                })?;
            let snapshot = iroha_fs::FileSnapshot::of(original.file(), true)
                .map_err(|error| Error::IO(error, path.clone()))?;
            let opened = super::secure_file_metadata::from_file(original.file())
                .map_err(|error| Error::IO(error, path.clone()))?;
            if !Self::sidecar_file_metadata_unchanged(&metadata.file, &opened)
                || opened.len() > limits.max_artifact_bytes.get() as u64
            {
                return Err(invalid(path, "FASTPQ temporary changed before removal"));
            }
            original
                .revalidate()
                .map_err(|error| Error::IO(error, path.clone()))?;
            drop(original);
            // Compare the same bounded original snapshot after opening the canonical
            // DELETE handle. A substitution in the sharing-mode transfer is refused.
            if !owner
                .remove_private_if_snapshot(TEMPORARY, &snapshot)
                .map_err(|error| Error::IO(error, path.clone()))?
            {
                return Err(invalid(
                    path,
                    "FASTPQ temporary disappeared before exact removal",
                ));
            }
        }
        Self::sync_bound_evidence_namespace(namespace, KIND)
    }
}

fn sync_artifact_file(file: &ArtifactDurabilityFile) -> std::io::Result<()> {
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

#[cfg(all(test, any(windows, all(unix, not(target_os = "espidf")))))]
mod tests {
    use super::*;
    use std::num::{NonZeroU64, NonZeroUsize};

    fn configured_fixture(policy: KuraFastpqArtifactPolicy) -> std::sync::Arc<Kura> {
        let mut kura = Kura::blank_kura_for_testing();
        std::sync::Arc::get_mut(&mut kura)
            .expect("unshared fixture construction")
            .fastpq_artifact_policy = policy;
        kura
    }

    fn limits(file: usize, count: usize, total: u64) -> KuraFastpqArtifactPolicy {
        KuraFastpqArtifactPolicy {
            max_artifact_bytes: NonZeroUsize::new(file).unwrap(),
            max_artifacts: NonZeroUsize::new(count).unwrap(),
            max_total_bytes: NonZeroU64::new(total).unwrap(),
        }
    }

    fn generous() -> KuraFastpqArtifactPolicy {
        limits(1024, 16, 16 * 1024)
    }

    fn path(kura: &Kura, reference: FastpqStoredArtifactReference) -> PathBuf {
        kura.store_root.join(DIRECTORY).join(reference.file_name())
    }

    fn write_fixture(kura: &Kura, path: &Path, bytes: &[u8]) {
        #[cfg(not(windows))]
        {
            let _ = kura;
            std::fs::write(path, bytes).unwrap();
        }
        #[cfg(windows)]
        {
            let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
            Kura::native_evidence_parent(&namespace, path)
                .unwrap()
                .write_atomic(
                    path.file_name().unwrap(),
                    bytes,
                    iroha_fs::PublishMode::Replace,
                )
                .unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_original_root_and_namespace_exclude_replacement() {
        let kura = configured_fixture(generous());
        let receipt = kura.persist_fastpq_artifact(b"abc").unwrap();
        let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
        assert!(
            std::fs::rename(&kura.store_root, kura.store_root.with_extension("moved")).is_err()
        );
        assert!(
            std::fs::rename(
                kura.store_root.join(DIRECTORY),
                kura.store_root.join("displaced")
            )
            .is_err()
        );
        assert!(Kura::progress_mutation_namespace_unchanged(&namespace));
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference()).unwrap(),
            b"abc"
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_inherited_and_foreign_private_artifacts_are_refused_without_repair() {
        for foreign_grant in [false, true] {
            let kura = configured_fixture(generous());
            let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
            let path = path(&kura, reference);
            if foreign_grant {
                assert!(
                    std::process::Command::new("icacls.exe")
                        .arg(&path)
                        .args(["/grant", "*S-1-1-0:R"])
                        .status()
                        .unwrap()
                        .success()
                );
            } else {
                std::fs::remove_file(&path).unwrap();
                std::fs::write(&path, b"abc").unwrap();
            }
            let before = std::process::Command::new("icacls.exe")
                .arg(&path)
                .output()
                .unwrap();
            assert!(kura.read_fastpq_artifact(reference).is_err());
            assert!(kura.persist_fastpq_artifact(b"abc").is_err());
            let after = std::process::Command::new("icacls.exe")
                .arg(&path)
                .output()
                .unwrap();
            assert_eq!(before.stdout, after.stdout);
            assert_eq!(std::fs::read(&path).unwrap(), b"abc");
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_hardlinks_and_junction_namespace_never_supply_artifact_authority() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        let target = path(&kura, reference);
        let alias = kura.store_root.join("alias");
        std::fs::hard_link(&target, &alias).unwrap();
        assert!(kura.read_fastpq_artifact(reference).is_err());
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(std::fs::read(&alias).unwrap(), b"abc");
        std::fs::remove_file(&alias).unwrap();
        assert_eq!(kura.read_fastpq_artifact(reference).unwrap(), b"abc");

        let kura = configured_fixture(generous());
        let outside = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/C", "mklink", "/J"])
                .arg(kura.store_root.join(DIRECTORY))
                .arg(outside.path())
                .status()
                .unwrap()
                .success()
        );
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[test]
    fn durable_roundtrip_and_idempotent_retry_keep_one_exact_artifact() {
        let kura = configured_fixture(generous());
        let bytes = b"opaque complete nominal proof wrapper";
        let first = kura.persist_fastpq_artifact(bytes).unwrap();
        let reference = first.reference();
        assert_eq!(reference.byte_len(), bytes.len() as u64);
        assert_eq!(reference.digest(), *Hash::new(bytes).as_ref());
        assert_eq!(kura.read_fastpq_artifact(reference).unwrap(), bytes);
        let second = kura.persist_fastpq_artifact(bytes).unwrap();
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
        let kura = configured_fixture(generous());
        let reference = FastpqStoredArtifactReference::for_bytes(b"absent");
        assert!(kura.read_fastpq_artifact(reference).is_err());
        assert!(!kura.store_root.join(DIRECTORY).exists());
    }

    #[test]
    fn receipt_issuer_is_distinct_even_for_identical_content_in_two_stores() {
        let first = configured_fixture(generous());
        let second = configured_fixture(generous());
        let left = first.persist_fastpq_artifact(b"abc").unwrap();
        let right = second.persist_fastpq_artifact(b"abc").unwrap();
        assert_eq!(left.reference(), right.reference());
        assert!(left.is_from(&first));
        assert!(!left.is_from(&second));
        assert!(right.is_from(&second));
        assert!(!right.is_from(&first));
    }

    #[test]
    fn empty_oversized_and_inconsistent_requests_fail_before_directory_creation() {
        for (bytes, policy) in [
            (&b""[..], generous()),
            (&b"too big"[..], limits(3, 1, 3)),
            (&b"a"[..], limits(8, 1, 4)),
            (&b"a"[..], limits(1, usize::MAX, 1)),
            (&b"a"[..], limits(usize::MAX, 1, u64::MAX)),
        ] {
            let kura = configured_fixture(policy);
            assert!(kura.persist_fastpq_artifact(bytes).is_err());
            assert!(!kura.store_root.join(DIRECTORY).exists());
        }
    }

    #[test]
    fn reference_caps_and_exact_lengths_are_checked() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        for length in [0, 2, 4, 1025, u64::MAX] {
            let changed =
                FastpqStoredArtifactReference::from_advertised(reference.digest(), length);
            assert!(
                kura.read_fastpq_artifact(changed).is_err(),
                "length {length}"
            );
        }
        assert_eq!(kura.read_fastpq_artifact(reference).unwrap(), b"abc");
    }

    #[test]
    fn advertised_hash_is_not_normalized_and_content_is_rehashed() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        for index in 0..32 {
            let mut digest = reference.digest();
            digest[index] ^= 1;
            let changed = FastpqStoredArtifactReference::from_advertised(digest, 3);
            assert_eq!(changed.digest(), digest);
            // Give the forged address the actual original bytes. Presence and
            // matching length are insufficient, including the hash marker bit.
            write_fixture(&kura, &path(&kura, changed), b"abc");
            assert!(kura.read_fastpq_artifact(changed).is_err());
            std::fs::remove_file(path(&kura, changed)).unwrap();
        }
    }

    #[test]
    fn changed_content_is_neither_read_nor_overwritten_on_retry() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        std::fs::write(path(&kura, reference), b"xyz").unwrap();
        assert!(kura.read_fastpq_artifact(reference).is_err());
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"xyz");
    }

    #[test]
    fn stable_count_quota_rejects_new_content_but_permits_exact_retry() {
        let policy = limits(8, 1, 16);
        let kura = configured_fixture(policy);
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        assert!(kura.persist_fastpq_artifact(b"def").is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"def")).exists());
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc").unwrap().reference(),
            reference
        );
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
    }

    #[test]
    fn aggregate_byte_quota_is_reserved_before_writing() {
        let policy = limits(4, 8, 6);
        let kura = configured_fixture(policy);
        let _first = kura.persist_fastpq_artifact(b"abc").unwrap();
        let _second = kura.persist_fastpq_artifact(b"def").unwrap();
        assert!(kura.persist_fastpq_artifact(b"g").is_err());
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
            let kura = configured_fixture(generous());
            let _initial = kura.persist_fastpq_artifact(b"initial").unwrap();
            let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
            write_fixture(&kura, &temp, temporary);
            let result = kura
                .persist_fastpq_artifact(b"fresh-verified-caller-bytes")
                .unwrap();
            assert!(!temp.exists());
            if !temporary.is_empty() {
                assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(temporary)).exists());
            }
            assert_eq!(
                kura.read_fastpq_artifact(result.reference()).unwrap(),
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
            let kura = configured_fixture(policy);
            let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
            let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
            write_fixture(&kura, &temp, temporary);
            assert!(kura.persist_fastpq_artifact(b"x").is_err());
            assert!(kura.read_fastpq_artifact(reference).is_err());
            assert_eq!(std::fs::read(&temp).unwrap(), temporary);
        }
    }

    #[test]
    fn unknown_names_subdirectories_and_too_many_entries_fail_closed() {
        for name in ["unrecognized", "ABC.norito", "../not-used"] {
            let kura = configured_fixture(generous());
            let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
            let directory = kura.store_root.join(DIRECTORY);
            if name == "../not-used" {
                std::fs::create_dir(directory.join("child")).unwrap();
            } else {
                std::fs::write(directory.join(name), b"x").unwrap();
            }
            assert!(kura.read_fastpq_artifact(reference).is_err());
            assert!(kura.persist_fastpq_artifact(b"new").is_err());
        }
        let kura = configured_fixture(limits(1024, 1, 16384));
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        write_fixture(
            &kura,
            &path(&kura, FastpqStoredArtifactReference::for_bytes(b"def")),
            b"def",
        );
        assert!(kura.read_fastpq_artifact(reference).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn symlink_and_hardlink_artifacts_are_rejected_without_touching_targets() {
        use std::os::unix::fs::symlink;
        for symlink_attack in [true, false] {
            let kura = configured_fixture(generous());
            let outside = tempfile::tempdir().unwrap();
            let target = outside.path().join("target");
            std::fs::write(&target, b"abc").unwrap();
            let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
            std::fs::remove_file(path(&kura, reference)).unwrap();
            if symlink_attack {
                symlink(&target, path(&kura, reference)).unwrap();
            } else {
                std::fs::hard_link(&target, path(&kura, reference)).unwrap();
            }
            assert!(kura.read_fastpq_artifact(reference).is_err());
            assert!(kura.persist_fastpq_artifact(b"abc").is_err());
            assert_eq!(std::fs::read(&target).unwrap(), b"abc");
        }
    }

    #[test]
    #[cfg(unix)]
    fn symlink_directory_and_temporary_are_never_followed_or_removed() {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        let kura = configured_fixture(generous());
        symlink(outside.path(), kura.store_root.join(DIRECTORY)).unwrap();
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);

        let kura = configured_fixture(generous());
        let _initial = kura.persist_fastpq_artifact(b"abc").unwrap();
        let target = outside.path().join("target");
        std::fs::write(&target, b"private").unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        symlink(&target, &temp).unwrap();
        assert!(kura.persist_fastpq_artifact(b"def").is_err());
        assert!(
            std::fs::symlink_metadata(&temp)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"private");
    }

    #[test]
    #[cfg(unix)]
    fn retained_kura_root_identity_rejects_a_replaced_root() {
        let kura = configured_fixture(generous());
        let root = kura.store_root.clone();
        let moved = root.with_extension("moved-fastpq-test");
        std::fs::rename(&root, &moved).unwrap();
        std::fs::create_dir(&root).unwrap();
        let result = kura.persist_fastpq_artifact(b"abc");
        let empty = std::fs::read_dir(&root).unwrap().count() == 0;
        std::fs::remove_dir(&root).unwrap();
        std::fs::rename(&moved, &root).unwrap();
        assert!(result.is_err());
        assert!(empty);
    }

    #[test]
    fn existing_file_requires_a_fresh_file_sync_before_every_receipt() {
        let kura = configured_fixture(generous());
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        for _ in 0..2 {
            FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
            assert!(kura.persist_fastpq_artifact(b"abc").is_err());
            assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"abc");
        }
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc").unwrap().reference(),
            reference
        );
    }

    #[test]
    fn publication_and_retry_directory_sync_failures_never_mint_receipts() {
        let kura = configured_fixture(generous());
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        for _ in 0..2 {
            super::super::FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(true));
            assert!(kura.persist_fastpq_artifact(b"abc").is_err());
            assert_eq!(std::fs::read(path(&kura, reference)).unwrap(), b"abc");
        }
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc").unwrap().reference(),
            reference
        );
    }

    #[test]
    fn ancestor_sync_failure_is_also_retried_for_existing_content() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        super::super::fail_progress_sidecar_ancestor_sync_for_tests(0, 1);
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc").unwrap().reference(),
            reference
        );
    }

    #[test]
    fn temporary_removal_sync_failure_stops_fresh_publication() {
        let kura = configured_fixture(generous());
        let _initial = kura.persist_fastpq_artifact(b"abc").unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        write_fixture(&kura, &temp, b"partial");
        super::super::FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(true));
        assert!(kura.persist_fastpq_artifact(b"def").is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"def")).exists());
        let receipt = kura.persist_fastpq_artifact(b"def").unwrap();
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference()).unwrap(),
            b"def"
        );
    }

    #[test]
    fn artifact_mutations_invalidate_the_total_disk_usage_cache() {
        let kura = configured_fixture(generous());
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        let enforced_before = kura.refresh_disk_usage_bytes().unwrap();
        let _receipt = kura.persist_fastpq_artifact(b"abc").unwrap();
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
        assert_eq!(
            kura.refresh_disk_usage_bytes().unwrap(),
            enforced_before + 3
        );
    }

    #[test]
    fn failed_publication_counts_retained_bytes_without_minting_a_receipt() {
        let kura = configured_fixture(generous());
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
        let _retry = kura.persist_fastpq_artifact(b"abc").unwrap();
        assert_eq!(kura.refresh_total_disk_usage_bytes().unwrap(), before + 3);
    }

    #[test]
    fn discarded_temporary_bytes_are_removed_from_global_disk_usage() {
        let kura = configured_fixture(generous());
        let _initial = kura.persist_fastpq_artifact(b"abc").unwrap();
        let temp = kura.store_root.join(DIRECTORY).join(TEMPORARY);
        write_fixture(&kura, &temp, b"partial");
        let before = kura.refresh_total_disk_usage_bytes().unwrap();
        let _next = kura.persist_fastpq_artifact(b"de").unwrap();
        assert!(!temp.exists());
        assert_eq!(
            kura.refresh_total_disk_usage_bytes().unwrap(),
            before - 7 + 2
        );
    }

    #[test]
    fn multi_megabyte_artifact_roundtrips_at_exact_capacity_in_separate_store() {
        let bytes = vec![0xA5; 8 * 1024 * 1024 + 37];
        let policy = limits(bytes.len(), 1, bytes.len() as u64);
        let kura = configured_fixture(policy);
        let receipt = kura.persist_fastpq_artifact(&bytes).unwrap();
        assert_eq!(receipt.reference().byte_len(), bytes.len() as u64);
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference()).unwrap(),
            bytes
        );
        assert!(kura.persist_fastpq_artifact(b"another").is_err());
        assert_eq!(kura.fastpq_proof_queue_len_for_testing(), 0);
    }

    #[test]
    fn configured_capacity_refuses_content_exceeding_remaining_disk_budget() {
        let mut kura = configured_fixture(generous());
        let before = kura.refresh_disk_usage_bytes().unwrap();
        // The artifact policy permits the content, but the disk budget does not.
        std::sync::Arc::get_mut(&mut kura)
            .unwrap()
            .max_disk_usage_bytes = before.checked_add(2).unwrap();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert!(!path(&kura, reference).exists());
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
        assert_eq!(kura.refresh_disk_usage_bytes().unwrap(), before);
    }

    #[test]
    fn configured_capacity_accepts_exact_fit_and_retry_but_rejects_new_content() {
        let mut kura = configured_fixture(generous());
        let before = kura.refresh_disk_usage_bytes().unwrap();
        let limit = before.checked_add(3).unwrap();
        std::sync::Arc::get_mut(&mut kura)
            .unwrap()
            .max_disk_usage_bytes = limit;
        let first = kura.persist_fastpq_artifact(b"abc").unwrap();
        let retry = kura.persist_fastpq_artifact(b"abc").unwrap();
        assert_eq!(first.reference(), retry.reference());
        assert_eq!(kura.refresh_disk_usage_bytes().unwrap(), before + 3);
        assert!(kura.persist_fastpq_artifact(b"d").is_err());
        assert!(!path(&kura, FastpqStoredArtifactReference::for_bytes(b"d")).exists());
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
        assert_eq!(
            kura.read_fastpq_artifact(first.reference()).unwrap(),
            b"abc"
        );
    }

    #[test]
    fn emergency_startup_cannot_publish_or_restore_artifact_evidence() {
        let kura = Kura::blank_kura_for_testing_in_emergency_fast_mode();
        let reference = FastpqStoredArtifactReference::for_bytes(b"abc");
        assert!(kura.persist_fastpq_artifact(b"abc").is_err());
        assert!(kura.read_fastpq_artifact(reference).is_err());
        assert!(!kura.store_root.join(DIRECTORY).exists());
    }

    fn observe_actual_physical_fixture(kura: &Kura) -> super::super::IndexResourceCounts {
        let counts = kura
            .physical_resource_scope()
            .unwrap()
            .observe(kura.evidence_resource_limits())
            .unwrap();
        let values = super::super::PHYSICAL_RESOURCE_FAMILIES
            .iter()
            .map(|family| (*family, counts[*family as usize]))
            .collect::<Vec<_>>();
        kura.resource_inventory
            .initialize(
                kura.resource_inventory.reconciliation_generation().unwrap(),
                &values,
            )
            .unwrap();
        counts
    }

    #[test]
    fn immutable_resources_count_empty_and_partial_temporary_cleanup_with_actual_bytes() {
        use super::super::ResourceFamily;
        for temporary_bytes in [Vec::new(), vec![9_u8; 4]] {
            let kura = configured_fixture(limits(8, 2, 16));
            let _first = kura.persist_fastpq_artifact(b"abc").unwrap();
            let temporary = kura.store_root.join(DIRECTORY).join(TEMPORARY);
            write_fixture(&kura, &temporary, &temporary_bytes);
            let before = observe_actual_physical_fixture(&kura);
            let first_evidence = before[ResourceFamily::EvidenceKeyRecords as usize];
            assert!(
                first_evidence.persisted_entries >= 2,
                "an empty pending.tmp still occupies a record"
            );
            assert_eq!(
                first_evidence.temporary_index_bytes,
                temporary_bytes.len() as u64
            );
            let receipt = kura.persist_fastpq_artifact(b"de").unwrap();
            let evidence = kura
                .resource_inventory
                .component_usage_for_tests(ResourceFamily::EvidenceKeyRecords)
                .unwrap();
            assert_eq!(evidence.persisted_entries, first_evidence.persisted_entries);
            assert_eq!(evidence.index_bytes, first_evidence.index_bytes + 2);
            assert_eq!(evidence.temporary_index_bytes, 0);
            assert_eq!(
                kura.resource_inventory
                    .component_usage_for_tests(ResourceFamily::StorageBytes)
                    .unwrap()
                    .storage_bytes,
                before[ResourceFamily::StorageBytes as usize].storage_bytes + 2
                    - temporary_bytes.len() as u64
            );
            assert!(!temporary.exists());
            assert_eq!(
                std::fs::read_dir(kura.store_root.join(DIRECTORY))
                    .unwrap()
                    .count(),
                2
            );
            assert_eq!(
                kura.read_fastpq_artifact(receipt.reference()).unwrap(),
                b"de"
            );
            let _retry = kura.persist_fastpq_artifact(b"de").unwrap();
            assert_eq!(
                kura.resource_inventory
                    .component_usage_for_tests(ResourceFamily::EvidenceKeyRecords)
                    .unwrap(),
                evidence
            );
        }
    }

    #[test]
    fn failed_artifact_sync_keeps_physical_resources_unavailable_until_complete_reaudit() {
        use super::super::ResourceFamily;
        let kura = configured_fixture(generous());
        observe_actual_physical_fixture(&kura);
        FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
        assert!(
            kura.persist_fastpq_artifact(b"retained after failed barrier")
                .is_err()
        );
        let reference = FastpqStoredArtifactReference::for_bytes(b"retained after failed barrier");
        assert_eq!(
            std::fs::read(path(&kura, reference)).unwrap(),
            b"retained after failed barrier"
        );
        for family in [
            ResourceFamily::EvidenceKeyRecords,
            ResourceFamily::StorageBytes,
        ] {
            assert!(
                kura.resource_inventory
                    .component_usage_for_tests(family)
                    .is_err()
            );
        }
        let receipt = kura
            .persist_fastpq_artifact(b"retained after failed barrier")
            .unwrap();
        assert_eq!(receipt.reference(), reference);
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(ResourceFamily::EvidenceKeyRecords)
                .is_err(),
            "a later successful receipt cannot erase an earlier accounting fault"
        );
        kura.validate_fastpq_artifact_inventory_on_startup(
            &mut kura.sidecar_lock.defer_notifications(),
        )
        .unwrap();
        let observed = observe_actual_physical_fixture(&kura);
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(ResourceFamily::EvidenceKeyRecords)
                .unwrap(),
            observed[ResourceFamily::EvidenceKeyRecords as usize]
        );
    }

    #[test]
    fn producer_and_source_share_one_aggregate_outbox_and_retry_identity() {
        let kura = configured_fixture(limits(8, 2, 8));
        let key = Hash::new(b"original full position");
        let carrier = iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"carrier"));
        let first = kura
            .persist_fastpq_source_record(2, carrier, b"tape")
            .unwrap();
        let produced = kura.persist_fastpq_producer_record(key, b"proof");
        assert!(
            produced.is_err(),
            "combined source + artifact exceeds the SAME byte cap"
        );
        assert!(kura.read_fastpq_producer_record(key).unwrap().is_none());
        let produced = kura.persist_fastpq_producer_record(key, b"artf").unwrap();
        assert!(first.is_from(&kura));
        assert!(produced.is_from(&kura));
        assert_eq!(
            kura.read_fastpq_producer_record(key).unwrap().unwrap(),
            b"artf"
        );
        assert_eq!(
            kura.persist_fastpq_producer_record(key, b"artf")
                .unwrap()
                .reference(),
            produced.reference()
        );
        assert!(kura.persist_fastpq_producer_record(key, b"evil").is_err());
        assert_eq!(
            kura.read_fastpq_producer_record(key).unwrap().unwrap(),
            b"artf"
        );
        assert!(
            kura.persist_fastpq_artifact(b"x").is_err(),
            "content-addressed attachments share both limits too"
        );
        assert!(!kura.store_root.join(DIRECTORY).join(TEMPORARY).exists());
        kura.validate_fastpq_artifact_inventory_on_startup(
            &mut kura.sidecar_lock.defer_notifications(),
        )
        .unwrap();
        let counts = observe_actual_physical_fixture(&kura);
        let evidence = counts[super::super::ResourceFamily::EvidenceKeyRecords as usize];
        assert!(evidence.persisted_entries >= 2);
    }

    #[test]
    fn producer_record_requires_original_durability_barriers_on_every_retry() {
        let kura = configured_fixture(generous());
        let key = Hash::new(b"source position");
        for _ in 0..2 {
            FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
            assert!(
                kura.persist_fastpq_producer_record(key, b"exact artifact")
                    .is_err()
            );
            // Observable bytes are only an advertisement after a failed durability barrier.
            assert_eq!(
                kura.read_fastpq_producer_record(key).unwrap().unwrap(),
                b"exact artifact"
            );
        }
        let receipt = kura
            .persist_fastpq_producer_record(key, b"exact artifact")
            .unwrap();
        assert!(receipt.is_from(&kura));
        assert_eq!(
            receipt.reference().digest(),
            *Hash::new(b"exact artifact").as_ref()
        );
        let other = configured_fixture(generous());
        assert!(!receipt.is_from(&other));
    }

    #[test]
    fn canonical_outbox_names_accept_only_exact_source_positions() {
        let hash = hex::encode(Hash::new(b"position").as_ref());
        assert!(stable_name(&format!("{hash}.producer")));
        assert!(stable_name(&format!("00000000000000000002-{hash}.fqs")));
        for invalid in [
            format!("00000000000000000000-{hash}.fqs"),
            format!("2-{hash}.fqs"),
            format!("00000000000000000002-{}.fqs", hash.to_uppercase()),
            format!("{hash}.producer.extra"),
            "cursor.json".into(),
        ] {
            assert!(!stable_name(&invalid), "{invalid}");
        }
    }

    #[test]
    fn actual_restart_retains_all_outbox_objects_and_enforces_aggregate_caps() {
        let directory = tempfile::tempdir().unwrap();
        let config = actual_config(directory.path(), limits(8, 2, 8));
        let key = Hash::new(b"complete source");
        let carrier = iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"native carrier"));
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &iroha_config::parameters::actual::LaneConfig::default(),
        )
        .unwrap();
        let source_receipt = kura
            .persist_fastpq_source_record(2, carrier, b"tape")
            .unwrap();
        assert!(source_receipt.is_from(&kura));
        assert_eq!(
            source_receipt.reference(),
            FastpqStoredArtifactReference::for_bytes(b"tape")
        );
        let receipt = kura.persist_fastpq_producer_record(key, b"artf").unwrap();
        drop(kura);
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &iroha_config::parameters::actual::LaneConfig::default(),
        )
        .unwrap();
        assert!(
            !receipt.is_from(&kura),
            "old receipts cannot authorize a new Kura instance"
        );
        assert!(
            !source_receipt.is_from(&kura),
            "source transport receipts also retain their original Kura identity"
        );
        assert_eq!(
            kura.read_fastpq_producer_record(key).unwrap().unwrap(),
            b"artf"
        );
        assert!(kura.persist_fastpq_artifact(b"x").is_err());
        assert_eq!(
            kura.persist_fastpq_producer_record(key, b"artf")
                .unwrap()
                .reference(),
            receipt.reference()
        );
    }

    fn actual_config(
        root: &Path,
        policy: KuraFastpqArtifactPolicy,
    ) -> iroha_config::parameters::actual::Kura {
        use iroha_config::parameters::defaults::kura as defaults;
        iroha_config::parameters::actual::Kura {
            init_mode: iroha_config::kura::InitMode::Strict,
            store_dir: iroha_config::base::WithOrigin::inline(root.to_path_buf()),
            max_disk_usage_bytes: defaults::MAX_DISK_USAGE_BYTES,
            blocks_in_memory: defaults::BLOCKS_IN_MEMORY,
            native_context_archive_max_bytes:
                iroha_config::parameters::defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
            history_checkpoint_cache_capacity:
                iroha_config::parameters::defaults::kura::HISTORY_CHECKPOINT_CACHE_CAPACITY,
            block_hash_history_bytes:
                iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
            transaction_history_bytes:
                iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
            membership_storage: iroha_config::parameters::defaults::kura::MEMBERSHIP_STORAGE_POLICY,
            fastpq_artifacts: policy,
            debug_output_new_blocks: false,
            fsync_mode: defaults::FSYNC_MODE,
            fsync_interval: defaults::FSYNC_INTERVAL,
        }
    }

    #[test]
    fn configured_startup_applies_retained_count_and_aggregate_caps_before_accepting_store() {
        for invalid in [
            "stable-count",
            "aggregate-with-temp",
            "per-artifact",
            "known-marker-name",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let config = actual_config(directory.path(), limits(8, 1, 8));
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
                &config,
                &iroha_config::parameters::actual::LaneConfig::default(),
            )
            .unwrap();
            let first = kura.persist_fastpq_artifact(b"12345678").unwrap();
            drop(kura);
            let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
                &config,
                &iroha_config::parameters::actual::LaneConfig::default(),
            )
            .expect("a retained namespace exactly at its configured caps reopens");
            assert_eq!(
                kura.read_fastpq_artifact(first.reference()).unwrap(),
                b"12345678"
            );
            let namespace = kura.store_root.join(DIRECTORY);
            match invalid {
                "stable-count" => {
                    write_fixture(
                        &kura,
                        &namespace.join(FastpqStoredArtifactReference::for_bytes(b"x").file_name()),
                        b"x",
                    );
                }
                "aggregate-with-temp" => {
                    write_fixture(&kura, &namespace.join(TEMPORARY), b"x");
                }
                "per-artifact" => {
                    std::fs::write(path(&kura, first.reference()), b"123456789").unwrap();
                }
                "known-marker-name" => {
                    std::fs::write(namespace.join(super::super::COUNT_FILE_NAME), b"x").unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                kura.validate_fastpq_artifact_inventory_on_startup(
                    &mut kura.sidecar_lock.defer_notifications()
                )
                .is_err(),
                "{invalid}"
            );
            let inventory_before = std::fs::read_dir(&namespace)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), std::fs::read(entry.path()).unwrap())
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            drop(kura);
            let reopened = Kura::open_test_kura_with_configured_lane_config(
                &config,
                &iroha_config::parameters::actual::LaneConfig::default(),
            );
            assert!(reopened.is_err(), "{invalid}");
            let error = reopened.expect_err("invalid FASTPQ inventory fails startup");
            assert!(
                matches!(&error, Error::IO(source, _) if source.to_string().contains("FASTPQ")),
                "{invalid}: {error}"
            );
            let inventory_after = std::fs::read_dir(&namespace)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), std::fs::read(entry.path()).unwrap())
                })
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(
                inventory_after, inventory_before,
                "startup quota rejection never deletes retained evidence"
            );
        }
    }

    #[test]
    fn configured_constructor_rejects_invalid_policy_before_creating_store() {
        let parent = tempfile::tempdir().unwrap();
        let store = parent.path().join("must-remain-absent");
        let config = actual_config(&store, limits(8, 1, 7));
        assert!(
            Kura::open_test_kura_with_configured_lane_config(
                &config,
                &iroha_config::parameters::actual::LaneConfig::default()
            )
            .is_err()
        );
        assert!(!store.exists());
    }

    #[cfg(windows)]
    #[test]
    fn native_final_inventory_borrows_the_exact_durability_original() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        let path = path(&kura, reference);
        let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
        let owner = Kura::native_evidence_parent(&namespace, &path).unwrap();
        let original = owner
            .open_private_durability(path.file_name().unwrap())
            .unwrap();
        let before = original.snapshot().unwrap();
        // The ordinary reader remains strict; the final scan must borrow our original.
        assert!(
            kura.inventory_fastpq_artifacts(&namespace, generous())
                .is_err()
        );
        let inventory = kura
            .inventory_fastpq_artifacts_with_original(
                &namespace,
                generous(),
                Some((&path, &original)),
            )
            .unwrap();
        assert_eq!(inventory.stable_count, 1);
        assert_eq!(inventory.total_bytes, 3);
        assert!(!inventory.temporary);
        assert_eq!(original.snapshot().unwrap(), before);
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(original);
        assert_eq!(kura.read_fastpq_artifact(reference).unwrap(), b"abc");
        assert_eq!(
            kura.persist_fastpq_artifact(b"abc").unwrap().reference(),
            reference
        );
    }

    #[cfg(windows)]
    #[test]
    fn native_borrowed_inventory_keeps_other_entries_strict_and_quotas_exact() {
        let kura = configured_fixture(generous());
        let first = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        let second = kura.persist_fastpq_artifact(b"defg").unwrap().reference();
        let first_path = path(&kura, first);
        let second_path = path(&kura, second);
        let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
        let owner = Kura::native_evidence_parent(&namespace, &first_path).unwrap();
        let original = owner
            .open_private_durability(first_path.file_name().unwrap())
            .unwrap();
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&second_path)
            .unwrap();
        assert!(
            kura.inventory_fastpq_artifacts_with_original(
                &namespace,
                generous(),
                Some((&first_path, &original)),
            )
            .is_err()
        );
        drop(writer);
        let inventory = kura
            .inventory_fastpq_artifacts_with_original(
                &namespace,
                generous(),
                Some((&first_path, &original)),
            )
            .unwrap();
        assert_eq!(inventory.stable_count, 2);
        assert_eq!(inventory.total_bytes, 7);
        assert!(
            kura.inventory_fastpq_artifacts_with_original(
                &namespace,
                limits(1024, 1, 1024),
                Some((&first_path, &original)),
            )
            .is_err()
        );
        assert!(
            kura.inventory_fastpq_artifacts_with_original(
                &namespace,
                limits(6, 16, 6),
                Some((&first_path, &original)),
            )
            .is_err()
        );
        original.revalidate().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn native_borrowed_inventory_refuses_other_original_and_late_acl_change() {
        let kura = configured_fixture(generous());
        let first = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        let second = kura.persist_fastpq_artifact(b"def").unwrap().reference();
        let first_path = path(&kura, first);
        let second_path = path(&kura, second);
        let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
        let owner = Kura::native_evidence_parent(&namespace, &first_path).unwrap();
        let original = owner
            .open_private_durability(first_path.file_name().unwrap())
            .unwrap();
        let wrong_name = kura
            .inventory_fastpq_artifacts_with_original(
                &namespace,
                generous(),
                Some((&second_path, &original)),
            )
            .err()
            .expect("another leaf cannot borrow this original");
        assert!(matches!(wrong_name, Error::IO(ref error, _) if error.raw_os_error() != Some(32)));
        assert!(
            std::process::Command::new("icacls.exe")
                .arg(&first_path)
                .args(["/grant", "*S-1-1-0:R"])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(
            kura.inventory_fastpq_artifacts_with_original(
                &namespace,
                generous(),
                Some((&first_path, &original)),
            )
            .is_err()
        );
        assert!(original.revalidate().is_err());
        drop(original);
        assert_eq!(std::fs::read(first_path).unwrap(), b"abc");
        assert_eq!(std::fs::read(second_path).unwrap(), b"def");
    }

    #[cfg(windows)]
    #[test]
    fn native_durability_retry_preserves_sync_refusal_and_exact_original() {
        let kura = configured_fixture(generous());
        let reference = kura.persist_fastpq_artifact(b"abc").unwrap().reference();
        let path = path(&kura, reference);
        let before = iroha_fs::RetainedFile::open_private(&path)
            .unwrap()
            .snapshot()
            .unwrap();
        let namespace = kura.open_fastpq_artifact_namespace(false).unwrap();
        FAIL_NEXT_FILE_SYNC.with(|flag| flag.set(true));
        let error = kura
            .finish_native_fastpq_object(&namespace, &path, b"abc", generous())
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("injected FASTPQ artifact file sync failure")
        );
        assert!(
            kura.finish_native_fastpq_object(&namespace, &path, b"bad", generous())
                .is_err()
        );
        kura.finish_native_fastpq_object(&namespace, &path, b"abc", generous())
            .unwrap();
        assert_eq!(
            iroha_fs::RetainedFile::open_private(&path)
                .unwrap()
                .snapshot()
                .unwrap(),
            before
        );
        assert_eq!(kura.read_fastpq_artifact(reference).unwrap(), b"abc");
    }

    #[test]
    fn constructed_policy_cannot_be_replaced_by_later_config_changes() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = actual_config(directory.path(), limits(8, 2, 16));
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &iroha_config::parameters::actual::LaneConfig::default(),
        )
        .unwrap();
        config.fastpq_artifacts = limits(16, 4, 64);
        assert!(config.fastpq_artifacts.validate().is_ok());
        assert!(kura.persist_fastpq_artifact(b"ninebytes").is_err());
        let receipt = kura.persist_fastpq_artifact(b"eightbyt").unwrap();
        assert_eq!(
            kura.read_fastpq_artifact(receipt.reference()).unwrap(),
            b"eightbyt"
        );
        assert_eq!(kura.fastpq_artifact_policy, limits(8, 2, 16));
    }
}

/// Original physical notification custody for retained FASTPQ artifact APIs.
#[cfg(all(test, unix, not(target_os = "espidf")))]
mod original_release_tests {
    include!("fastpq_artifact_release_tests.rs");
}
