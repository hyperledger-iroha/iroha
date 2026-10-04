//! Portable software custody for the existing provider-attestation journal and local inventory.
//!
//! Files provide crash recovery and exclusive process ownership, not an external rollback seal.
//! Every approval and use still needs the daemon's original native capture and governed authority.
//! Inventory receipts attest local retention only; the publisher independently registers and
//! proves each attestation through its existing archive-manager transaction journal.

use crate::{provider_attestation_journal::*, provider_ingest_runtime::ProviderIngestFutureV1};
use iroha_data_model::{
    NetworkId, musubi::MusubiProviderBundleAttestationKeyV1, sorafs::capacity::ProviderId,
};
use iroha_fs::{FileIdentity, FileSnapshot, OwnerDirectory, PrivateDirectory, PublishMode};
use norito::{
    NoritoSchema,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use std::{
    fs::File,
    io::{self, Read as _},
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const DIRECTORY: &str = "provider-attestation-native";
const LOCK: &str = "owner.lock";
const JOURNAL: &str = "journal.nrt";
const CLOCK: &str = "clock.nrt";
const INVENTORY: &str = "inventory.nrt";
/// Fixed identity of the native, local-retention inventory adapter.
pub const NATIVE_PROVIDER_ATTESTATION_INVENTORY_HANDLE_V1: &str =
    "software://sorafs/provider-attestation-inventory";
/// Fixed identity of native crash-durable host UTC; this is not an external time seal.
pub const NATIVE_PROVIDER_ATTESTATION_CLOCK_HANDLE_V1: &str =
    "software://sorafs/provider-attestation-clock";
/// Fixed purpose of native dedicated completion-key approval; it signs no transaction.
pub const NATIVE_PROVIDER_ATTESTATION_APPROVAL_HANDLE_V1: &str =
    "software://sorafs/provider-attestation-approval";
const FRAME_OVERHEAD: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "sorafs_node::provider_attestation_native::BindingV1")]
struct Binding {
    network_id: NetworkId,
    provider_id: ProviderId,
    policy_digest: [u8; 32],
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "sorafs_node::provider_attestation_native::JournalFileV1")]
struct JournalFile {
    binding: Binding,
    checkpoint: Option<Vec<u8>>,
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "sorafs_node::provider_attestation_native::ClockFileV1")]
struct ClockFile {
    binding: Binding,
    floor_unix_ms: u64,
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "sorafs_node::provider_attestation_native::InventoryEntryV1")]
struct InventoryEntry {
    revision: u64,
    item: MusubiProviderAttestationInventoryItemV1,
}
#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "sorafs_node::provider_attestation_native::InventoryFileV1")]
struct InventoryFile {
    binding: Binding,
    entries: Vec<InventoryEntry>,
}

/// One exact native journal and its local coordinator inventory, sharing retained private custody.
///
/// Clones of the runtime or inventory retain the same ownership lock. This object is not proof of
/// current signer eligibility, native registration, storage health or publication readiness.
pub struct NativeMusubiProviderAttestationCustodyV1 {
    runtime: Arc<MusubiProviderAttestationJournalRuntimeV1>,
    inventory: Arc<NativeMusubiProviderAttestationInventoryV1>,
}
impl std::fmt::Debug for NativeMusubiProviderAttestationCustodyV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMusubiProviderAttestationCustodyV1")
            .finish_non_exhaustive()
    }
}
impl NativeMusubiProviderAttestationCustodyV1 {
    /// Initialize an absent fixed namespace during explicit provisioning.
    ///
    /// # Errors
    /// Refuses existing history, unsafe paths, invalid scope, bounds or uncertain publication.
    /// Ordinary startup must call [`Self::open`] and never recreate missing initialized files.
    pub fn initialize(
        root: &Path,
        network_id: NetworkId,
        provider_id: ProviderId,
        policy: MusubiProviderAttestationJournalPolicyV1,
    ) -> io::Result<()> {
        let binding = binding(network_id, provider_id, policy)?;
        let private_parent = PrivateDirectory::open(root)?;
        let parent = OwnerDirectory::open(root)?;
        if parent.identity()? != private_parent.identity()? {
            return Err(rejected());
        }
        norito::with_decode_limits_scope(limits_for(policy), || {
            let maximum = policy.checkpoint_max_bytes.saturating_add(FRAME_OVERHEAD);
            let journal = initial_frame(
                &JournalFile {
                    binding,
                    checkpoint: None,
                },
                maximum,
            )?;
            let clock = initial_frame(
                &ClockFile {
                    binding,
                    floor_unix_ms: unix_ms()?,
                },
                maximum,
            )?;
            let inventory = initial_frame(
                &InventoryFile {
                    binding,
                    entries: Vec::new(),
                },
                maximum,
            )?;
            private_parent.revalidate()?;
            let directory = parent.publish_private_child(
                DIRECTORY,
                &[
                    (LOCK, &[]),
                    (JOURNAL, &journal),
                    (CLOCK, &clock),
                    (INVENTORY, &inventory),
                ],
            )?;
            private_parent.revalidate()?;
            let shared = NativeStore::lock(directory, binding, policy)?;
            shared.validate_all()
        })
    }

    /// Reopen all exact original files and bind the existing journal state machine.
    ///
    /// # Errors
    /// Refuses missing or substituted history, a competing owner, foreign scope or corrupt bytes.
    pub fn open(
        root: &Path,
        network_id: NetworkId,
        provider_id: ProviderId,
        policy: MusubiProviderAttestationJournalPolicyV1,
    ) -> io::Result<Self> {
        let binding = binding(network_id, provider_id, policy)?;
        let parent = PrivateDirectory::open(root)?;
        let directory = parent.open_child(DIRECTORY)?;
        let shared = Arc::new(NativeStore::lock(directory, binding, policy)?);
        shared.validate_all()?;
        let runtime = MusubiProviderAttestationJournalRuntimeV1::new_native_opened(
            shared.clone(),
            policy,
            shared.clone(),
            network_id,
            provider_id,
        )
        .map_err(|_| rejected())?;
        Ok(Self {
            runtime: Arc::new(runtime),
            inventory: Arc::new(NativeMusubiProviderAttestationInventoryV1 { shared }),
        })
    }
    /// Retain the sole original journal for the bounded opaque capture driver.
    pub fn runtime(&self) -> Arc<MusubiProviderAttestationJournalRuntimeV1> {
        self.runtime.clone()
    }
    /// Retain the same local inventory for governed handoff and publication-coordinator reads.
    pub fn inventory(&self) -> Arc<NativeMusubiProviderAttestationInventoryV1> {
        self.inventory.clone()
    }
}

#[derive(Clone)]
pub(crate) struct NativeStore {
    inner: Arc<NativeCustody>,
}
impl std::ops::Deref for NativeStore {
    type Target = NativeCustody;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
pub(crate) struct NativeCustody {
    directory: PrivateDirectory,
    lock: File,
    lock_identity: FileIdentity,
    binding: Binding,
    policy: MusubiProviderAttestationJournalPolicyV1,
    gate: Mutex<()>,
    jobs: Arc<tokio::sync::Semaphore>,
}
impl std::fmt::Debug for NativeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeProviderAttestationStore")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}
impl NativeStore {
    fn lock(
        directory: PrivateDirectory,
        binding: Binding,
        policy: MusubiProviderAttestationJournalPolicyV1,
    ) -> io::Result<Self> {
        let lock = directory.open_existing_lock(LOCK)?;
        lock.try_lock().map_err(io::Error::other)?;
        let lock_identity = FileIdentity::of(&lock)?;
        Ok(Self {
            inner: Arc::new(NativeCustody {
                directory,
                lock,
                lock_identity,
                binding,
                policy,
                gate: Mutex::new(()),
                jobs: Arc::new(tokio::sync::Semaphore::new(1)),
            }),
        })
    }

    fn revalidate(&self) -> io::Result<()> {
        self.directory.revalidate()?;
        if FileIdentity::of(&self.lock)? != self.lock_identity
            || FileIdentity::of(&self.directory.open_read(LOCK)?)? != self.lock_identity
        {
            return Err(rejected());
        }
        Ok(())
    }
    fn maximum(&self) -> usize {
        self.policy
            .checkpoint_max_bytes
            .saturating_add(FRAME_OVERHEAD)
    }
    fn limits(&self) -> norito::DecodeLimits {
        limits_for(self.policy)
    }
    async fn run_blocking<T, E, F>(&self, unavailable: E, operation: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: Send + Copy + 'static,
        F: FnOnce(&Self) -> Result<T, E> + Send + 'static,
    {
        // Debit the complete bounded scratch allowance before crossing a thread boundary. The
        // worker owns that prepaid allowance; nested phases share it and never replenish it.
        let limits = self.limits();
        norito::core::reserve_decode_allocation(limits.max_total_allocated_bytes())
            .map_err(|_| unavailable)?;
        let permit = self
            .jobs
            .clone()
            .try_acquire_owned()
            .map_err(|_| unavailable)?;
        let store = self.clone();
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| unavailable)?;
        runtime
            .spawn_blocking(move || {
                let _permit = permit;
                norito::with_decode_limits_scope(limits, || operation(&store))
            })
            .await
            .map_err(|_| unavailable)?
    }
    fn read<T>(&self, name: &str) -> io::Result<T>
    where
        T: for<'a> norito::core::NoritoDeserialize<'a> + norito::core::NoritoSerialize,
    {
        norito::with_decode_limits_scope(self.limits(), || {
            self.revalidate()?;
            let mut file = self.directory.open_read(name)?;
            let before = FileSnapshot::of(&file, true)?;
            let length = usize::try_from(file.metadata()?.len()).map_err(|_| rejected())?;
            if length > self.maximum() {
                return Err(rejected());
            }
            norito::core::reserve_decode_allocation(length).map_err(io::Error::other)?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(length).map_err(io::Error::other)?;
            bytes.resize(length, 0);
            file.read_exact(&mut bytes)?;
            if file.read(&mut [0_u8; 1])? != 0
                || FileSnapshot::of(&file, true)? != before
                || FileSnapshot::of(&self.directory.open_read(name)?, true)? != before
            {
                return Err(rejected());
            }
            let limits = self.limits();
            let sequence = if name == JOURNAL {
                self.maximum()
            } else {
                self.policy.max_entries.max(4096)
            };
            let limits = norito::DecodeLimits::new(
                sequence,
                limits.max_field_bytes(),
                limits.max_total_elements(),
                limits.max_total_allocated_bytes(),
                limits.max_nesting_depth(),
            );
            let value =
                norito::decode_canonical_with_limits(&bytes, limits).map_err(io::Error::other)?;
            self.revalidate()?;
            Ok(value)
        })
    }
    fn encode<T: norito::core::NoritoSerialize>(
        &self,
        name: &str,
        value: &T,
        mode: PublishMode,
    ) -> io::Result<()> {
        norito::with_decode_limits_scope(self.limits(), || {
            self.revalidate()?;
            let length = norito::canonical_frame_len(value).map_err(io::Error::other)?;
            if length > self.maximum() {
                return Err(rejected());
            }
            norito::core::reserve_decode_allocation(length).map_err(io::Error::other)?;
            let bytes = norito::encode_canonical(value).map_err(io::Error::other)?;
            self.directory.write_atomic(name, &bytes, mode)?;
            self.revalidate()
        })
    }
    fn journal(&self) -> io::Result<JournalFile> {
        norito::with_decode_limits_scope(self.limits(), || {
            let journal: JournalFile = self.read(JOURNAL)?;
            if journal.binding != self.binding {
                return Err(rejected());
            }
            if let Some(bytes) = &journal.checkpoint {
                validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
                    bytes,
                    self.policy,
                    &self.binding.network_id,
                    self.binding.provider_id,
                )
                .map_err(io::Error::other)?;
            }
            Ok(journal)
        })
    }
    fn inventory_file(&self) -> io::Result<InventoryFile> {
        norito::with_decode_limits_scope(self.limits(), || {
            let value: InventoryFile = self.read(INVENTORY)?;
            if value.binding != self.binding || value.entries.len() > self.policy.max_entries {
                return Err(rejected());
            }
            for (index, entry) in value.entries.iter().enumerate() {
                if entry.revision
                    != u64::try_from(index)
                        .ok()
                        .and_then(|i| i.checked_add(1))
                        .ok_or_else(rejected)?
                {
                    return Err(rejected());
                }
                self.validate_item(&entry.item)?;
            }
            let bytes = value
                .entries
                .len()
                .checked_mul(std::mem::size_of::<MusubiProviderBundleAttestationKeyV1>())
                .ok_or_else(rejected)?;
            norito::core::reserve_decode_allocation(bytes).map_err(io::Error::other)?;
            let mut keys = Vec::new();
            keys.try_reserve_exact(value.entries.len())
                .map_err(io::Error::other)?;
            keys.extend(value.entries.iter().map(|entry| entry.item.key()));
            keys.sort_unstable();
            if keys.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(rejected());
            }
            Ok(value)
        })
    }
    fn validate_item(&self, item: &MusubiProviderAttestationInventoryItemV1) -> io::Result<()> {
        item.validate().map_err(io::Error::other)?;
        if item.attestation().payload.binding.network_id != self.binding.network_id
            || item.key().provider_id != self.binding.provider_id
        {
            return Err(rejected());
        }
        Ok(())
    }
    fn clock(&self) -> io::Result<ClockFile> {
        norito::with_decode_limits_scope(self.limits(), || {
            let value: ClockFile = self.read(CLOCK)?;
            if value.binding != self.binding || value.floor_unix_ms == 0 {
                return Err(rejected());
            }
            Ok(value)
        })
    }
    fn validate_all(&self) -> io::Result<()> {
        norito::with_decode_limits_scope(self.limits(), || {
            let journal = self.journal()?;
            let clock = self.clock()?;
            if let Some(bytes) = journal.checkpoint {
                let (_, floor) =
                    validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
                        &bytes,
                        self.policy,
                        &self.binding.network_id,
                        self.binding.provider_id,
                    )
                    .map_err(io::Error::other)?;
                if floor > clock.floor_unix_ms {
                    return Err(rejected());
                }
            }
            self.inventory_file()?;
            self.revalidate()
        })
    }
    pub(crate) async fn now_unix_ms(
        self: &Arc<Self>,
    ) -> Result<u64, MusubiProviderAttestationJournalErrorV1> {
        self.run_blocking(
            MusubiProviderAttestationJournalErrorV1::ClockUnavailable,
            move |store| {
                let _gate = store
                    .gate
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?;
                let mut record = store
                    .clock()
                    .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?;
                let now = unix_ms()
                    .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?;
                if now < record.floor_unix_ms {
                    return Err(MusubiProviderAttestationJournalErrorV1::ClockRollback);
                }
                if now > record.floor_unix_ms {
                    record.floor_unix_ms = now;
                    store
                        .encode(CLOCK, &record, PublishMode::Replace)
                        .map_err(|_| MusubiProviderAttestationJournalErrorV1::ClockUnavailable)?;
                }
                Ok(now)
            },
        )
        .await
    }
}

fn limits_for(policy: MusubiProviderAttestationJournalPolicyV1) -> norito::DecodeLimits {
    let maximum = policy.checkpoint_max_bytes.saturating_add(FRAME_OVERHEAD);
    norito::DecodeLimits::new(
        maximum,
        maximum,
        maximum.saturating_mul(2),
        maximum.saturating_mul(8),
        64,
    )
}
fn initial_frame<T: norito::core::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> io::Result<Vec<u8>> {
    let length = norito::canonical_frame_len(value).map_err(io::Error::other)?;
    if length > maximum {
        return Err(rejected());
    }
    norito::core::reserve_decode_allocation(length).map_err(io::Error::other)?;
    norito::encode_canonical(value).map_err(io::Error::other)
}

fn binding(
    network_id: NetworkId,
    provider_id: ProviderId,
    policy: MusubiProviderAttestationJournalPolicyV1,
) -> io::Result<Binding> {
    if network_id.as_bytes()[31] & 1 != 1 || *provider_id.as_bytes() == [0; 32] {
        return Err(rejected());
    }
    Ok(Binding {
        network_id,
        provider_id,
        policy_digest: policy.digest().map_err(io::Error::other)?,
    })
}
fn unix_ms() -> io::Result<u64> {
    let value = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis(),
    )
    .map_err(io::Error::other)?;
    if value == 0 || value == u64::MAX {
        return Err(rejected());
    }
    Ok(value)
}
fn rejected() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "native attestation custody rejected",
    )
}

impl MusubiProviderAttestationJournalStoreV1 for NativeStore {
    fn load<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationJournalStoreSnapshotV1,
            MusubiProviderAttestationJournalStoreErrorV1,
        >,
    > {
        Box::pin(self.run_blocking(
            MusubiProviderAttestationJournalStoreErrorV1::Unavailable,
            move |store| {
                let _gate = store
                    .gate
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                let journal = store
                    .journal()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                match journal.checkpoint {
                    Some(bytes) => {
                        MusubiProviderAttestationJournalStoreSnapshotV1::from_checkpoint_bytes(
                            bytes,
                        )
                    }
                    None => Ok(MusubiProviderAttestationJournalStoreSnapshotV1::empty()),
                }
            },
        ))
    }
    fn compare_and_swap<'a>(
        &'a self,
        expected_revision: Option<[u8; 32]>,
        replacement: Vec<u8>,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            MusubiProviderAttestationJournalCasOutcomeV1,
            MusubiProviderAttestationJournalStoreErrorV1,
        >,
    > {
        Box::pin(self.run_blocking(
            MusubiProviderAttestationJournalStoreErrorV1::Unavailable,
            move |store| {
                let _gate = store
                    .gate
                    .lock()
                    .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                norito::with_decode_limits_scope(store.limits(), || {
                    let (next_sequence, next_floor) =
                        validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
                            &replacement,
                            store.policy,
                            &store.binding.network_id,
                            store.binding.provider_id,
                        )
                        .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                    let original = store
                        .journal()
                        .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Rejected)?;
                    let actual_revision = original
                        .checkpoint
                        .as_deref()
                        .map(musubi_provider_attestation_journal_checkpoint_revision_v1);
                    let revision =
                        musubi_provider_attestation_journal_checkpoint_revision_v1(&replacement);
                    if original.checkpoint.as_deref() == Some(replacement.as_slice()) {
                        return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored {
                            revision,
                        });
                    }
                    if actual_revision != expected_revision {
                        return Ok(MusubiProviderAttestationJournalCasOutcomeV1::Conflict);
                    }
                    let (sequence, floor) = match original.checkpoint.as_deref() {
                        None => (0, 0),
                        Some(bytes) => {
                            validate_musubi_provider_attestation_journal_checkpoint_metadata_v1(
                                bytes,
                                store.policy,
                                &store.binding.network_id,
                                store.binding.provider_id,
                            )
                            .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Rejected)?
                        }
                    };
                    if sequence.checked_add(1) != Some(next_sequence)
                        || next_floor < floor
                        || store
                            .clock()
                            .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Rejected)?
                            .floor_unix_ms
                            < next_floor
                    {
                        return Err(MusubiProviderAttestationJournalStoreErrorV1::Rejected);
                    }
                    store
                        .encode(
                            JOURNAL,
                            &JournalFile {
                                binding: store.binding,
                                checkpoint: Some(replacement),
                            },
                            PublishMode::Replace,
                        )
                        .map_err(|_| MusubiProviderAttestationJournalStoreErrorV1::Unavailable)?;
                    Ok(MusubiProviderAttestationJournalCasOutcomeV1::Stored { revision })
                })
            },
        ))
    }
}

/// Immutable local coordinator inventory retained by the same native custody owner.
///
/// This adapter's receipts prove durable local retention only, never native registration or
/// current eligibility. The daemon must wrap it in the existing governed scope adapter, and the
/// publication coordinator must use the exact same retained instance for its reads.
pub struct NativeMusubiProviderAttestationInventoryV1 {
    shared: Arc<NativeStore>,
}
impl std::fmt::Debug for NativeMusubiProviderAttestationInventoryV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeMusubiProviderAttestationInventoryV1")
            .finish_non_exhaustive()
    }
}
impl MusubiProviderAttestationInventorySinkV1 for NativeMusubiProviderAttestationInventoryV1 {
    fn put<'a>(
        &'a self,
        item: MusubiProviderAttestationInventoryItemV1,
    ) -> ProviderIngestFutureV1<'a, Result<u64, MusubiProviderAttestationInventoryErrorV1>> {
        Box::pin(async move {
            self.shared
                .run_blocking(
                    MusubiProviderAttestationInventoryErrorV1::Unavailable,
                    move |store| {
                        let _gate = store
                            .gate
                            .lock()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                        norito::with_decode_limits_scope(store.limits(), || {
                            store.validate_item(&item).map_err(|_| {
                                MusubiProviderAttestationInventoryErrorV1::InvalidItem
                            })?;
                            let mut inventory = store
                                .inventory_file()
                                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            if let Some(original) = inventory.entries.iter().find(|entry| {
                                entry.item.scope() == item.scope() && entry.item.key() == item.key()
                            }) {
                                return if original.item == item {
                                    Ok(original.revision)
                                } else {
                                    Err(MusubiProviderAttestationInventoryErrorV1::Conflict)
                                };
                            }
                            if inventory.entries.len() >= store.policy.max_entries {
                                return Err(MusubiProviderAttestationInventoryErrorV1::Rejected);
                            }
                            let revision = u64::try_from(inventory.entries.len())
                                .ok()
                                .and_then(|value| value.checked_add(1))
                                .ok_or(MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            let next_len = inventory
                                .entries
                                .len()
                                .checked_add(1)
                                .ok_or(MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            let capacity_bytes = next_len
                                .checked_mul(std::mem::size_of::<InventoryEntry>())
                                .ok_or(MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            norito::core::reserve_decode_allocation(capacity_bytes)
                                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            let mut entries = Vec::new();
                            entries
                                .try_reserve_exact(next_len)
                                .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                            entries.extend(inventory.entries);
                            inventory.entries = entries;
                            inventory.entries.push(InventoryEntry { revision, item });
                            store
                                .encode(INVENTORY, &inventory, PublishMode::Replace)
                                .map_err(|_| {
                                    MusubiProviderAttestationInventoryErrorV1::Unavailable
                                })?;
                            Ok(revision)
                        })
                    },
                )
                .await
        })
    }
}
impl MusubiProviderAttestationInventoryReaderV1 for NativeMusubiProviderAttestationInventoryV1 {
    fn get<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
        key: MusubiProviderBundleAttestationKeyV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryReadbackV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    > {
        Box::pin(async move {
            scope.validate()?;
            if scope.network_id != self.shared.binding.network_id {
                return Err(MusubiProviderAttestationInventoryErrorV1::Rejected);
            }
            let scope = scope.clone();
            self.shared
                .run_blocking(
                    MusubiProviderAttestationInventoryErrorV1::Unavailable,
                    move |store| {
                        key.validate()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::InvalidItem)?;
                        if key.provider_id != store.binding.provider_id
                            || key.archive_id != scope.archive_id
                            || key.replication_order != scope.replication_order
                        {
                            return Err(MusubiProviderAttestationInventoryErrorV1::Rejected);
                        }
                        let _gate = store
                            .gate
                            .lock()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                        let inventory = store
                            .inventory_file()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                        inventory
                            .entries
                            .into_iter()
                            .find(|entry| entry.item.scope() == &scope && entry.item.key() == key)
                            .map(|entry| {
                                MusubiProviderAttestationInventoryReadbackV1::try_new(
                                    entry.item,
                                    entry.revision,
                                )
                            })
                            .transpose()
                    },
                )
                .await
        })
    }
    fn inventory<'a>(
        &'a self,
        scope: &'a MusubiProviderAttestationInventoryScopeV1,
    ) -> ProviderIngestFutureV1<
        'a,
        Result<
            Option<MusubiProviderAttestationInventoryV1>,
            MusubiProviderAttestationInventoryErrorV1,
        >,
    > {
        Box::pin(async move {
            scope.validate()?;
            if scope.network_id != self.shared.binding.network_id {
                return Err(MusubiProviderAttestationInventoryErrorV1::Rejected);
            }
            let scope = scope.clone();
            self.shared
                .run_blocking(
                    MusubiProviderAttestationInventoryErrorV1::Unavailable,
                    move |store| {
                        let _gate = store
                            .gate
                            .lock()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Unavailable)?;
                        let inventory = store
                            .inventory_file()
                            .map_err(|_| MusubiProviderAttestationInventoryErrorV1::Rejected)?;
                        // This custody owns one provider: an archive/order has at most one immutable item.
                        inventory
                            .entries
                            .into_iter()
                            .find(|entry| entry.item.scope() == &scope)
                            .map(|entry| {
                                let mut items = Vec::new();
                                norito::core::reserve_decode_allocation(std::mem::size_of::<
                                    MusubiProviderAttestationInventoryItemV1,
                                >(
                                ))
                                .map_err(|_| {
                                    MusubiProviderAttestationInventoryErrorV1::Unavailable
                                })?;
                                items.try_reserve_exact(1).map_err(|_| {
                                    MusubiProviderAttestationInventoryErrorV1::Unavailable
                                })?;
                                items.push(entry.item);
                                MusubiProviderAttestationInventoryV1::new(scope.clone(), items)
                            })
                            .transpose()
                    },
                )
                .await
        })
    }
}
impl MusubiProviderAttestationInventoryRuntimeV1 for NativeMusubiProviderAttestationInventoryV1 {
    fn runtime_handle(&self) -> &str {
        NATIVE_PROVIDER_ATTESTATION_INVENTORY_HANDLE_V1
    }
    fn qualification(
        &self,
    ) -> Result<
        MusubiProviderAttestationInventoryQualificationV1,
        MusubiProviderAttestationInventoryRuntimeErrorV1,
    > {
        Ok(MusubiProviderAttestationInventoryQualificationV1::new(
            1,
            self.shared.binding.policy_digest,
        ))
    }
    fn check_readiness<'a>(
        &'a self,
    ) -> ProviderIngestFutureV1<'a, Result<(), MusubiProviderAttestationInventoryRuntimeErrorV1>>
    {
        Box::pin(async move {
            self.shared
                .run_blocking(
                    MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable,
                    move |store| {
                        let _gate = store.gate.lock().map_err(|_| {
                            MusubiProviderAttestationInventoryRuntimeErrorV1::Unavailable
                        })?;
                        store
                            .validate_all()
                            .map_err(|_| MusubiProviderAttestationInventoryRuntimeErrorV1::Rejected)
                    },
                )
                .await
        })
    }
}

#[cfg(test)]
mod tests;
