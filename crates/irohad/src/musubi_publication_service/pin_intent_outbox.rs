//! Immutable, bounded signed pin-intent custody before Queue submission.
//!
//! A successful file-and-directory durability barrier must precede any effectful Queue call.
//! This owner has no Queue handle and cannot activate stock publication by itself.
// TODO: Qualify the checked stage -> native high-water finality -> Queue handoff and an
// independently current network checkpoint before daemon open. The local audit and checked
// coordinator do not grant a Queue capability.
use super::{
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    MusubiPublicationPinOutboxHighWaterReadErrorV1, MusubiPublicationPrivateServiceContextV1,
    pin_registration::validate_signed_pin_intent,
};
use iroha_config::parameters::{actual::MusubiPublicationPaidPinPolicy, defaults};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    musubi::MusubiPinOutboxHighWaterV1,
    sorafs::pin_registry::{
        ManifestDigest, SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1, StorageClass,
    },
    transaction::{DEFAULT_TRANSACTION_TIME_TO_LIVE, SignedTransaction},
};
use iroha_version::codec::DecodeVersioned as _;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, Stat};
use std::{
    ffi::OsString,
    fs::{self, File, Metadata},
    io::{Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Component, Path, PathBuf},
};

const OWNER_FILE: &str = "pin-intent-owner-v1.norito";
const INTENT_SUFFIX: &str = ".pin-intent";
const OWNER_MAGIC: [u8; 16] = *b"musubi-pin-own1\0";
const INTENT_MAGIC: [u8; 16] = *b"musubi-pin-int1\0";
const FRAME_HEADER_BYTES: usize = 16 + 4 + 32;
const MAX_OWNER_BYTES: usize = 8 * 1024;
const MAX_SIGNED_WIRE_BYTES: usize = 2 * 1024 * 1024;
const MAX_INTENT_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECORDS: u32 = 1_024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;

/// Closed operational failure for durable signed pin-intent custody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusubiPinIntentOutboxErrorV1 {
    /// Original finalized-history allocation refusal, retained for a local retry.
    Deferred(iroha_core::execution_attempt::ExecutionDeferred),
    /// Configuration, owner identity, file shape or retained record is invalid.
    Invalid,
    /// Another process owns the same directory.
    Locked,
    /// A different signed wire or source binding already occupies this manifest digest.
    Conflict,
    /// The immutable record or aggregate capacity is exhausted.
    Capacity,
    /// Local durable storage or allocation is unavailable.
    Unavailable,
    /// The source archive registration is ahead of this node's finalized State.
    LocallyAhead,
    /// The current directory has not been joined to authenticated finalized high-water.
    MissingFinalizedAnchor,
}
impl core::fmt::Display for MusubiPinIntentOutboxErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Deferred(_) => "signed pin-intent history read is waiting for local capacity",
            Self::Invalid => "signed pin-intent outbox state is invalid",
            Self::Locked => "signed pin-intent outbox is already owned",
            Self::Conflict => "signed pin-intent outbox has a conflicting record",
            Self::Capacity => "signed pin-intent outbox capacity is exhausted",
            Self::Unavailable => "signed pin-intent outbox storage is unavailable",
            Self::LocallyAhead => "signed pin-intent source is ahead of local finality",
            Self::MissingFinalizedAnchor => {
                "signed pin-intent outbox lacks a finalized monotonic rollback anchor"
            }
        })
    }
}
impl std::error::Error for MusubiPinIntentOutboxErrorV1 {}

/// Fixed lifetime file/count bounds for one private outbox owner.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "irohad::musubi_publication_service::MusubiPinIntentOutboxLimitsV1")]
pub struct MusubiPinIntentOutboxLimitsV1 {
    /// Maximum immutable signed intents retained over the deployment lifetime.
    pub max_records: u32,
    /// Maximum aggregate framed bytes, including the owner marker.
    pub max_total_bytes: u64,
}
impl MusubiPinIntentOutboxLimitsV1 {
    fn validate(self) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        if self.max_records == 0
            || self.max_records > MAX_RECORDS
            || self.max_total_bytes < MAX_INTENT_BYTES as u64
            || self.max_total_bytes > MAX_TOTAL_BYTES
        {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        Ok(())
    }
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "irohad::musubi_publication_service::PinnedPinIntentOwnerV1")]
struct PinnedPinIntentOwnerV1 {
    version: u8,
    network_id: NetworkId,
    pin_authority: AccountId,
    session_id: [u8; 32],
    storage_class: StorageClass,
    retention_horizon_secs: u64,
    limits: MusubiPinIntentOutboxLimitsV1,
}

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "irohad::musubi_publication_service::StoredSignedPinIntentV1")]
struct StoredSignedPinIntentV1 {
    version: u8,
    operation_id: [u8; 32],
    source: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    manifest_digest: ManifestDigest,
    signed_wire: Vec<u8>,
}

/// One exact signed V1 transaction recovered from durable custody.
#[derive(Debug)]
pub struct MusubiRecoveredSignedPinIntentV1 {
    /// Stable publication-operation identity.
    pub operation_id: [u8; 32],
    /// Source archive registration to recheck before dispatch or reconciliation.
    pub source: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    /// Canonical pin-manifest digest.
    pub manifest_digest: ManifestDigest,
    /// Exact signed transaction, including all authorization proofs.
    pub transaction: SignedTransaction,
}

/// Read-only result of joining complete local custody to current finalized State/Kura.
///
/// This is an audit observation, not authorization to sign, submit to Queue, or publish.
/// A coordinated rollback of State and Kura still requires an independent current network
/// checkpoint before any effectful publication coordinator can use this lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusubiPinIntentOutboxLocalAuditV1 {
    /// Immutable signing-session lineage.
    pub session_id: [u8; 32],
    /// Current native high-water revision.
    pub revision: u64,
    /// Digest of every retained owner and signed-intent frame.
    pub inventory_digest: [u8; 32],
    /// Number of immutable signed intents in the verified local inventory.
    pub retained_records: u32,
}

struct PinnedDirectoryV1 {
    name: OsString,
    file: File,
    metadata: Metadata,
}
struct PinnedPrivateRootV1 {
    lineage: Vec<PinnedDirectoryV1>,
    owner_uid: u32,
}

/// Process-exclusive immutable signed pin-intent outbox for one network and submitter.
///
/// Opening validates every retained record under bounded allocation. An interrupted `.pending`
/// file leaves the directory closed until audited repair; no ordinary retry silently discards a
/// possibly signed intent. Retained files are never deleted by this owner. The current process
/// detects online path substitution, while privileged offline rollback needs an external
/// deployment-sealed monotonic anchor before production activation.
pub struct DurableMusubiPinIntentOutboxV1 {
    owner: PinnedPinIntentOwnerV1,
    owner_frame_digest: [u8; 32],
    owner_frame_length: u64,
    root: PinnedPrivateRootV1,
    reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    records: Vec<(ManifestDigest, [u8; 32], u64, [u8; 32])>,
    total_bytes: u64,
}
impl core::fmt::Debug for DurableMusubiPinIntentOutboxV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DurableMusubiPinIntentOutboxV1")
            .field("network_id", &self.owner.network_id)
            .field("records", &self.records.len())
            .field("total_bytes", &self.total_bytes)
            .finish_non_exhaustive()
    }
}

impl MusubiPublicationPrivateServiceContextV1 {
    /// Audit one initialized local outbox against its current finalized native high-water.
    ///
    /// This opens the original owner and every immutable signed intent under a process lock,
    /// reauthenticates each source archive, then joins the complete inventory to the exact
    /// successful high-water advance in this daemon's State/Kura. It drops the outbox before
    /// returning and exposes no signed wire or Queue capability. Absence is never interpreted as
    /// permission to replace potentially lost signed intent. An external current network
    /// checkpoint and serialized Queue coordinator remain release gates.
    ///
    /// # Errors
    /// Rejects missing finalized high-water, changed custody, invalid finality, or unsafe I/O.
    pub fn audit_local_pin_intent_outbox(
        &self,
        root: &Path,
        policy: MusubiPublicationPaidPinPolicy,
        limits: MusubiPinIntentOutboxLimitsV1,
    ) -> Result<MusubiPinIntentOutboxLocalAuditV1, MusubiPinIntentOutboxErrorV1> {
        let outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            root,
            self.network_id(),
            policy,
            limits,
            self.finalized_archive_registration_reader(),
        )?;
        let reader = self.finalized_pin_outbox_high_water_reader();
        audit_finalized_inventory_with(&outbox, |authority| reader.read_current(authority))
    }

    /// Reopen an initialized private outbox under this daemon's exact State/Kura ownership.
    ///
    /// State/Kura now carry the signed pin-outbox high-water and authenticate its latest advance.
    /// The production coordinator still lacks a serialized stage, finality, compare and Queue
    /// protocol. A locally valid restored directory can omit an already staged signed transaction,
    /// so this constructor stays closed until it joins its complete inventory to that finality.
    ///
    /// # Errors
    /// Returns [`MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor`] until the daemon verifies
    /// the current finalized high-water against this exact directory before exposing custody.
    pub fn open_pin_intent_outbox(
        &self,
        _root: &Path,
        _policy: MusubiPublicationPaidPinPolicy,
        _limits: MusubiPinIntentOutboxLimitsV1,
    ) -> Result<DurableMusubiPinIntentOutboxV1, MusubiPinIntentOutboxErrorV1> {
        Err(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor)
    }
}

impl DurableMusubiPinIntentOutboxV1 {
    /// Provision one previously empty owner-only directory and immutable deployment marker.
    ///
    /// `session_id` must be fresh for this signer/deployment lineage and durably retained by the
    /// marker. Reprovisioning after a lost directory is not an automatic recovery operation.
    ///
    /// Provisioning is separate from ordinary open; a missing or interrupted owner marker never
    /// becomes a fresh empty outbox during daemon startup.
    ///
    /// # Errors
    /// Refuses unsafe ancestry, nonempty storage, invalid limits or a failed durability barrier.
    pub fn initialize(
        root: &Path,
        network_id: NetworkId,
        session_id: [u8; 32],
        policy: MusubiPublicationPaidPinPolicy,
        limits: MusubiPinIntentOutboxLimitsV1,
    ) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        limits.validate()?;
        validate_configured_policy(&policy)?;
        if session_id == [0; 32] {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        let root = PinnedPrivateRootV1::open(root)?;
        if !root.list_names()?.is_empty() {
            return Err(MusubiPinIntentOutboxErrorV1::Conflict);
        }
        let owner = PinnedPinIntentOwnerV1 {
            version: 1,
            network_id,
            pin_authority: policy.transaction_authority,
            session_id,
            storage_class: policy.storage_class,
            retention_horizon_secs: policy.retention_horizon_secs,
            limits,
        };
        let bytes =
            norito::encode_canonical(&owner).map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        if bytes.is_empty() || bytes.len() > MAX_OWNER_BYTES {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        root.write_frame(OWNER_FILE, OWNER_MAGIC, &bytes, MAX_OWNER_BYTES)?;
        Ok(())
    }

    pub(super) fn open_inner(
        root: &Path,
        network_id: NetworkId,
        policy: MusubiPublicationPaidPinPolicy,
        limits: MusubiPinIntentOutboxLimitsV1,
        reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    ) -> Result<Self, MusubiPinIntentOutboxErrorV1> {
        limits.validate()?;
        validate_configured_policy(&policy)?;
        let root = PinnedPrivateRootV1::open(root)?;
        let (owner_bytes, owner_frame_digest, owner_length) =
            root.read_frame(OWNER_FILE, OWNER_MAGIC, MAX_OWNER_BYTES)?;
        let owner: PinnedPinIntentOwnerV1 = decode_bounded(&owner_bytes)?;
        if owner.version != 1
            || owner.network_id != network_id
            || owner.session_id == [0; 32]
            || owner.pin_authority != policy.transaction_authority
            || owner.storage_class != policy.storage_class
            || owner.retention_horizon_secs != policy.retention_horizon_secs
            || owner.limits != limits
        {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(limits.max_records as usize)
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Unavailable)?;
        let mut total_bytes = owner_length;
        for name in root.list_names()? {
            if name == OWNER_FILE {
                continue;
            }
            let digest = digest_from_name(&name)?;
            let (bytes, frame_digest, length) =
                root.read_frame(&name, INTENT_MAGIC, MAX_INTENT_BYTES)?;
            let record: StoredSignedPinIntentV1 = decode_bounded(&bytes)?;
            if record.manifest_digest != digest {
                return Err(MusubiPinIntentOutboxErrorV1::Invalid);
            }
            let _ = validate_record(&owner, &record)?;
            verify_source_with_reader(&reader, &record.source)?;
            total_bytes = total_bytes
                .checked_add(length)
                .ok_or(MusubiPinIntentOutboxErrorV1::Capacity)?;
            if records.len() >= limits.max_records as usize || total_bytes > limits.max_total_bytes
            {
                return Err(MusubiPinIntentOutboxErrorV1::Capacity);
            }
            records.push((digest, frame_digest, length, record.operation_id));
        }
        records.sort_unstable_by_key(|row| row.0);
        if records.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        if records
            .iter()
            .enumerate()
            .any(|(index, row)| records[index + 1..].iter().any(|other| row.3 == other.3))
        {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        Ok(Self {
            owner,
            owner_frame_digest,
            owner_frame_length: owner_length,
            root,
            reader,
            records,
            total_bytes,
        })
    }

    /// Persist one exact signed pin transaction before any Queue submission is attempted.
    ///
    /// Exact retries are idempotent. A different signed wire, source query, or operation for the
    /// same manifest digest is a conflict. The source archive is reauthenticated immediately
    /// before the immutable file is committed.
    ///
    /// # Errors
    /// Refuses stale source finality, malformed signed intent, capacity, conflict or unsafe I/O.
    pub fn stage_signed_intent(
        &mut self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        manifest_digest: ManifestDigest,
        transaction: &SignedTransaction,
    ) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        self.verify_inventory()?;
        if operation_id == [0; 32] || source.network_id != self.owner.network_id {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        self.verify_source(source)?;
        let manifest = validate_signed_pin_intent(
            &self.owner.network_id,
            &self.owner.pin_authority,
            &source.registration.commitment,
            transaction,
            manifest_digest,
        )
        .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        validate_manifest_policy(&self.owner, &manifest, transaction)?;
        let signed_wire = transaction
            .encode_wire_v1()
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        if signed_wire.is_empty() || signed_wire.len() > MAX_SIGNED_WIRE_BYTES {
            return Err(MusubiPinIntentOutboxErrorV1::Capacity);
        }
        let record = StoredSignedPinIntentV1 {
            version: 1,
            operation_id,
            source: source.clone(),
            manifest_digest,
            signed_wire,
        };
        let bytes =
            norito::encode_canonical(&record).map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        if bytes.is_empty() || bytes.len() > MAX_INTENT_BYTES {
            return Err(MusubiPinIntentOutboxErrorV1::Capacity);
        }
        let name = intent_name(manifest_digest);
        match self
            .records
            .binary_search_by_key(&manifest_digest, |row| row.0)
        {
            Ok(index) => {
                let (resident, digest, _) =
                    self.root
                        .read_frame(&name, INTENT_MAGIC, MAX_INTENT_BYTES)?;
                if digest == self.records[index].1 && resident == bytes {
                    return Ok(());
                }
                return Err(MusubiPinIntentOutboxErrorV1::Conflict);
            }
            Err(_) => {}
        }
        if self.records.iter().any(|row| row.3 == operation_id) {
            return Err(MusubiPinIntentOutboxErrorV1::Conflict);
        }
        let frame_length = u64::try_from(FRAME_HEADER_BYTES + bytes.len())
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Capacity)?;
        if self.records.len() >= self.owner.limits.max_records as usize
            || self
                .total_bytes
                .checked_add(frame_length)
                .is_none_or(|total| total > self.owner.limits.max_total_bytes)
        {
            return Err(MusubiPinIntentOutboxErrorV1::Capacity);
        }
        let frame_digest = self
            .root
            .write_frame(&name, INTENT_MAGIC, &bytes, MAX_INTENT_BYTES)?;
        let position = self
            .records
            .binary_search_by_key(&manifest_digest, |row| row.0)
            .unwrap_err();
        self.records.insert(
            position,
            (manifest_digest, frame_digest, frame_length, operation_id),
        );
        self.total_bytes += frame_length;
        Ok(())
    }

    /// List retained manifest digests without materializing signed transactions.
    ///
    /// The returned count is bounded by the deployment-fixed immutable limit. A caller should
    /// use [`Self::recover_signed_intent`] one digest at a time on restart.
    ///
    /// # Errors
    /// Refuses a substituted directory or record before returning names.
    pub fn retained_digests(&self) -> Result<Vec<ManifestDigest>, MusubiPinIntentOutboxErrorV1> {
        self.verify_inventory()?;
        let mut digests = Vec::new();
        digests
            .try_reserve_exact(self.records.len())
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Unavailable)?;
        for (digest, _, _, _) in &self.records {
            digests.push(*digest);
        }
        Ok(digests)
    }

    /// Recover one immutable signed transaction after refreshing its source finality.
    ///
    /// This returns the exact original signed wire for a qualified coordinator to reconcile
    /// against the pin-finality reader before any same-wire Queue retry. It never signs a fresh
    /// transaction and never claims that a queued transaction has executed.
    ///
    /// # Errors
    /// Refuses online substitution, invalid signed wire, stale source, or unavailable storage.
    pub fn recover_signed_intent(
        &self,
        manifest_digest: ManifestDigest,
    ) -> Result<MusubiRecoveredSignedPinIntentV1, MusubiPinIntentOutboxErrorV1> {
        self.verify_inventory()?;
        let index = self
            .records
            .binary_search_by_key(&manifest_digest, |row| row.0)
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        let name = intent_name(manifest_digest);
        let (bytes, frame_digest, length) =
            self.root
                .read_frame(&name, INTENT_MAGIC, MAX_INTENT_BYTES)?;
        if frame_digest != self.records[index].1 || length != self.records[index].2 {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        let record: StoredSignedPinIntentV1 = decode_bounded(&bytes)?;
        if record.manifest_digest != manifest_digest || record.operation_id != self.records[index].3
        {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        let transaction = validate_record(&self.owner, &record)?;
        self.verify_source(&record.source)?;
        Ok(MusubiRecoveredSignedPinIntentV1 {
            operation_id: record.operation_id,
            source: record.source,
            manifest_digest,
            transaction,
        })
    }

    /// Recover the original signed wire for one *expected* immutable publication operation.
    ///
    /// Absence is a closed failure: a valid-looking older outbox directory might have omitted a
    /// signed intent that was already dispatched. A coordinator cannot infer from local absence
    /// that it is safe to sign a replacement. A retained operation is fully revalidated against
    /// its source archive. This does not detect rollback of the entire valid local directory;
    /// that requires an independently anchored monotonic outbox lineage before production use.
    ///
    /// # Errors
    /// Refuses a zero or absent operation id, substituted custody, or stale finalized source.
    pub fn recover_operation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<MusubiRecoveredSignedPinIntentV1, MusubiPinIntentOutboxErrorV1> {
        self.verify_inventory()?;
        if operation_id == [0; 32] {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        let (digest, _, _, _) = self
            .records
            .iter()
            .find(|(_, _, _, retained_operation)| *retained_operation == operation_id)
            .ok_or(MusubiPinIntentOutboxErrorV1::Invalid)?;
        self.recover_signed_intent(*digest)
    }

    pub(super) fn owner_binding(&self) -> (&NetworkId, &AccountId) {
        (&self.owner.network_id, &self.owner.pin_authority)
    }

    /// Immutable session lineage provisioned with this owner marker.
    #[must_use]
    pub const fn session_id(&self) -> [u8; 32] {
        self.owner.session_id
    }

    /// Digest of the complete, reverified owner and immutable signed-intent inventory.
    ///
    /// This is the exact public commitment submitted through the native high-water instruction.
    /// It does not establish finality by itself; the finalized State/Kura reader must authenticate
    /// the matching revision before any Queue effect is allowed.
    ///
    /// # Errors
    /// Refuses online substitution, malformed files, or unavailable custody.
    pub fn inventory_digest(&self) -> Result<[u8; 32], MusubiPinIntentOutboxErrorV1> {
        self.inventory_digest_with_exclusion(None)
    }

    /// Recompute the predecessor inventory for exactly one retained operation.
    ///
    /// The checked stage coordinator uses this only to recognize an interrupted durable stage:
    /// one retained signed intent may be ahead of the current finalized native high-water.
    /// It cannot authorize Queue dispatch or infer that an absent operation was never signed.
    pub(super) fn inventory_digest_excluding_operation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<[u8; 32], MusubiPinIntentOutboxErrorV1> {
        if operation_id == [0; 32] || !self.records.iter().any(|row| row.3 == operation_id) {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        self.inventory_digest_with_exclusion(Some(operation_id))
    }

    fn inventory_digest_with_exclusion(
        &self,
        excluded_operation: Option<[u8; 32]>,
    ) -> Result<[u8; 32], MusubiPinIntentOutboxErrorV1> {
        self.verify_inventory()?;
        let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:pin-outbox-inventory:v1");
        hasher.update(&self.owner_frame_digest);
        hasher.update(&self.owner_frame_length.to_le_bytes());
        let count = self.records.len() - usize::from(excluded_operation.is_some());
        hasher.update(&(count as u32).to_le_bytes());
        for (manifest, frame_digest, frame_length, operation_id) in &self.records {
            if excluded_operation == Some(*operation_id) {
                continue;
            }
            hasher.update(manifest.as_bytes());
            hasher.update(frame_digest);
            hasher.update(&frame_length.to_le_bytes());
            hasher.update(operation_id);
        }
        Ok(*hasher.finalize().as_bytes())
    }

    /// Compare complete local custody with a separately authenticated current high-water.
    ///
    /// The caller must obtain `record` through the finalized State/Kura reader, not from a local
    /// marker or request. This deliberately grants no Queue or open capability on its own.
    pub fn verify_finalized_high_water(
        &self,
        record: &MusubiPinOutboxHighWaterV1,
    ) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        record
            .validate()
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
        if record.network_id != self.owner.network_id
            || record.pin_authority != self.owner.pin_authority
            || record.session_id != self.owner.session_id
            || record.inventory_digest != self.inventory_digest()?
        {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        Ok(())
    }

    pub(super) fn verify_requested_source(
        &self,
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        if source.network_id != self.owner.network_id {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        self.verify_source(source)
    }

    fn verify_source(
        &self,
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        verify_source_with_reader(&self.reader, source)
    }

    fn verify_inventory(&self) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::Invalid;
        let (owner_bytes, digest, length) =
            self.root
                .read_frame(OWNER_FILE, OWNER_MAGIC, MAX_OWNER_BYTES)?;
        if digest != self.owner_frame_digest
            || length != self.owner_frame_length
            || decode_bounded::<PinnedPinIntentOwnerV1>(&owner_bytes)? != self.owner
        {
            return Err(Invalid);
        }
        let names = self.root.list_names()?;
        if names.len() != self.records.len() + 1 {
            return Err(Invalid);
        }
        let mut seen_owner = false;
        for name in names {
            if name == OWNER_FILE {
                seen_owner = true;
                continue;
            }
            let manifest_digest = digest_from_name(&name)?;
            let index = self
                .records
                .binary_search_by_key(&manifest_digest, |row| row.0)
                .map_err(|_| Invalid)?;
            let (bytes, digest, length) =
                self.root
                    .read_frame(&name, INTENT_MAGIC, MAX_INTENT_BYTES)?;
            let record: StoredSignedPinIntentV1 = decode_bounded(&bytes)?;
            if digest != self.records[index].1
                || length != self.records[index].2
                || record.manifest_digest != manifest_digest
                || record.operation_id != self.records[index].3
            {
                return Err(Invalid);
            }
        }
        if !seen_owner {
            return Err(Invalid);
        }
        Ok(())
    }
}

fn audit_finalized_inventory_with(
    outbox: &DurableMusubiPinIntentOutboxV1,
    read_current: impl FnOnce(
        &AccountId,
    ) -> Result<
        Option<MusubiPinOutboxHighWaterV1>,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    >,
) -> Result<MusubiPinIntentOutboxLocalAuditV1, MusubiPinIntentOutboxErrorV1> {
    let record = read_current(&outbox.owner.pin_authority)
        .map_err(|error| match error {
            MusubiPublicationPinOutboxHighWaterReadErrorV1::Deferred(error) => {
                MusubiPinIntentOutboxErrorV1::Deferred(error)
            }
            MusubiPublicationPinOutboxHighWaterReadErrorV1::LocallyAhead => {
                MusubiPinIntentOutboxErrorV1::LocallyAhead
            }
            MusubiPublicationPinOutboxHighWaterReadErrorV1::Invalid => {
                MusubiPinIntentOutboxErrorV1::Invalid
            }
        })?
        .ok_or(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor)?;
    // Re-read every local frame after the State/Kura observation. A process-exclusive pinned
    // directory does not by itself protect against privileged mutation or offline restore.
    outbox.verify_finalized_high_water(&record)?;
    Ok(MusubiPinIntentOutboxLocalAuditV1 {
        session_id: record.session_id,
        revision: record.revision,
        inventory_digest: record.inventory_digest,
        retained_records: u32::try_from(outbox.records.len())
            .map_err(|_| MusubiPinIntentOutboxErrorV1::Capacity)?,
    })
}

fn validate_configured_policy(
    policy: &MusubiPublicationPaidPinPolicy,
) -> Result<(), MusubiPinIntentOutboxErrorV1> {
    if policy.retention_horizon_secs
        <= u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1)
        || policy.retention_horizon_secs
            > defaults::musubi_publication::MAX_PIN_RETENTION_HORIZON_SECS
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    Ok(())
}

fn validate_manifest_policy(
    owner: &PinnedPinIntentOwnerV1,
    manifest: &sorafs_manifest::ManifestV1,
    transaction: &SignedTransaction,
) -> Result<(), MusubiPinIntentOutboxErrorV1> {
    let storage_class = match owner.storage_class {
        StorageClass::Hot => sorafs_manifest::StorageClass::Hot,
        StorageClass::Warm => sorafs_manifest::StorageClass::Warm,
        StorageClass::Cold => sorafs_manifest::StorageClass::Cold,
    };
    let creation_ms = u64::try_from(transaction.creation_time().as_millis())
        .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
    let retention_epoch = creation_ms
        .div_ceil(1_000)
        .checked_add(DEFAULT_TRANSACTION_TIME_TO_LIVE.as_secs())
        .and_then(|epoch| epoch.checked_add(owner.retention_horizon_secs))
        .ok_or(MusubiPinIntentOutboxErrorV1::Invalid)?;
    if creation_ms == 0
        || transaction.time_to_live() != Some(DEFAULT_TRANSACTION_TIME_TO_LIVE)
        || manifest.pin_policy.storage_class != storage_class
        || manifest.pin_policy.min_replicas < 3
        || manifest.pin_policy.retention_epoch != retention_epoch
        || !manifest.alias_claims.is_empty()
        || !manifest.metadata.is_empty()
        || !manifest.governance.council_signatures.is_empty()
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    Ok(())
}

fn verify_source_with_reader(
    reader: &MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
) -> Result<(), MusubiPinIntentOutboxErrorV1> {
    let archive = reader
        .read_current_archive(source)
        .map_err(|error| match error {
            super::MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::Deferred(error) => {
                MusubiPinIntentOutboxErrorV1::Deferred(error)
            }
            super::MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::LocallyAhead => {
                MusubiPinIntentOutboxErrorV1::LocallyAhead
            }
            super::MusubiPublicationFinalizedArchiveRegistrationReadErrorV1::Invalid => {
                MusubiPinIntentOutboxErrorV1::Invalid
            }
        })?;
    if archive.registration_projection() != source.registration {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    Ok(())
}

fn validate_record(
    owner: &PinnedPinIntentOwnerV1,
    record: &StoredSignedPinIntentV1,
) -> Result<SignedTransaction, MusubiPinIntentOutboxErrorV1> {
    if record.version != 1
        || record.operation_id == [0; 32]
        || record.source.network_id != owner.network_id
        || record.signed_wire.is_empty()
        || record.signed_wire.len() > MAX_SIGNED_WIRE_BYTES
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    let transaction = SignedTransaction::decode_all_versioned(&record.signed_wire)
        .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
    if transaction
        .encode_wire_v1()
        .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?
        != record.signed_wire
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    let manifest = validate_signed_pin_intent(
        &owner.network_id,
        &owner.pin_authority,
        &record.source.registration.commitment,
        &transaction,
        record.manifest_digest,
    )
    .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
    validate_manifest_policy(owner, &manifest, &transaction)?;
    Ok(transaction)
}

fn intent_name(digest: ManifestDigest) -> String {
    format!("{}{}", hex::encode(digest.as_bytes()), INTENT_SUFFIX)
}
fn digest_from_name(name: &str) -> Result<ManifestDigest, MusubiPinIntentOutboxErrorV1> {
    let hex = name
        .strip_suffix(INTENT_SUFFIX)
        .ok_or(MusubiPinIntentOutboxErrorV1::Invalid)?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    let mut digest = [0_u8; 32];
    hex::decode_to_slice(hex, &mut digest).map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
    if digest == [0; 32] {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    Ok(ManifestDigest::new(digest))
}

fn decode_bounded<T>(bytes: &[u8]) -> Result<T, MusubiPinIntentOutboxErrorV1>
where
    T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>,
{
    let allocated = bytes
        .len()
        .checked_mul(8)
        .and_then(|size| size.checked_add(64 * 1024))
        .ok_or(MusubiPinIntentOutboxErrorV1::Invalid)?;
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(bytes.len(), bytes.len(), bytes.len(), allocated, 64),
    )
    .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)
}

impl PinnedPrivateRootV1 {
    fn open(path: &Path) -> Result<Self, MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::{Invalid, Locked, Unavailable};
        if !path.is_absolute() {
            return Err(Invalid);
        }
        let owner_uid = rustix::process::geteuid().as_raw();
        let slash = File::from(
            rustix::fs::open("/", directory_flags(), Mode::empty()).map_err(|_| Unavailable)?,
        );
        let slash_metadata = slash.metadata().map_err(|_| Unavailable)?;
        let mut lineage = vec![PinnedDirectoryV1 {
            name: OsString::from("/"),
            file: slash,
            metadata: slash_metadata,
        }];
        let mut reconstructed = PathBuf::from("/");
        for component in path.components() {
            let Component::Normal(name) = component else {
                if component == Component::RootDir {
                    continue;
                }
                return Err(Invalid);
            };
            reconstructed.push(name);
            let parent = &lineage.last().ok_or(Invalid)?.file;
            let named =
                rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|_| Invalid)?;
            let file = File::from(
                rustix::fs::openat(parent, name, directory_flags(), Mode::empty())
                    .map_err(|_| Invalid)?,
            );
            let metadata = file.metadata().map_err(|_| Unavailable)?;
            if !safe_ancestor(&metadata, owner_uid) || !stat_matches(&named, &metadata) {
                return Err(Invalid);
            }
            lineage.push(PinnedDirectoryV1 {
                name: name.to_owned(),
                file,
                metadata,
            });
        }
        if reconstructed != path || lineage.len() < 2 {
            return Err(Invalid);
        }
        let leaf = lineage.last().ok_or(Invalid)?;
        if leaf.metadata.uid() != owner_uid || leaf.metadata.mode() & 0o7777 != 0o700 {
            return Err(Invalid);
        }
        rustix::fs::flock(
            &leaf.file,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(|error| {
            if error == rustix::io::Errno::WOULDBLOCK {
                Locked
            } else {
                Unavailable
            }
        })?;
        Ok(Self { lineage, owner_uid })
    }

    fn root(&self) -> Result<&File, MusubiPinIntentOutboxErrorV1> {
        self.lineage
            .last()
            .map(|part| &part.file)
            .ok_or(MusubiPinIntentOutboxErrorV1::Invalid)
    }

    fn verify_lineage(&self) -> Result<(), MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::{Invalid, Unavailable};
        for (index, part) in self.lineage.iter().enumerate() {
            let current = part.file.metadata().map_err(|_| Unavailable)?;
            if !safe_ancestor(&current, self.owner_uid) || !same_file(&current, &part.metadata) {
                return Err(Invalid);
            }
            if index > 0 {
                let named = rustix::fs::statat(
                    &self.lineage[index - 1].file,
                    &part.name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| Invalid)?;
                if !stat_matches(&named, &current) {
                    return Err(Invalid);
                }
            }
        }
        Ok(())
    }

    fn list_names(&self) -> Result<Vec<String>, MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::{Invalid, Unavailable};
        self.verify_lineage()?;
        let entries = rustix::fs::Dir::read_from(self.root()?).map_err(|_| Unavailable)?;
        let mut names = Vec::new();
        names
            .try_reserve_exact(MAX_RECORDS as usize + 1)
            .map_err(|_| Unavailable)?;
        for entry in entries {
            let entry = entry.map_err(|_| Unavailable)?;
            let raw = entry.file_name().to_bytes();
            if matches!(raw, b"." | b"..") {
                continue;
            }
            let name = std::str::from_utf8(raw).map_err(|_| Invalid)?;
            if names.len() >= MAX_RECORDS as usize + 1 {
                return Err(MusubiPinIntentOutboxErrorV1::Capacity);
            }
            names.push(name.to_owned());
        }
        self.verify_lineage()?;
        Ok(names)
    }

    fn read_frame(
        &self,
        name: &str,
        magic: [u8; 16],
        max_payload: usize,
    ) -> Result<(Vec<u8>, [u8; 32], u64), MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::{Invalid, Unavailable};
        self.verify_lineage()?;
        let before = rustix::fs::statat(self.root()?, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Invalid)?;
        let mut file = File::from(
            rustix::fs::openat(
                self.root()?,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Invalid)?,
        );
        let opened = file.metadata().map_err(|_| Unavailable)?;
        if !stat_matches(&before, &opened)
            || !opened.is_file()
            || opened.nlink() != 1
            || opened.uid() != self.owner_uid
            || opened.mode() & 0o7777 != 0o400
            || opened.len() < FRAME_HEADER_BYTES as u64
            || opened.len() > (FRAME_HEADER_BYTES + max_payload) as u64
        {
            return Err(Invalid);
        }
        let mut header = [0_u8; FRAME_HEADER_BYTES];
        file.read_exact(&mut header).map_err(read_error)?;
        let length =
            u32::from_be_bytes(header[16..20].try_into().expect("fixed frame header")) as usize;
        if header[..16] != magic
            || length == 0
            || length > max_payload
            || opened.len() != (FRAME_HEADER_BYTES + length) as u64
        {
            return Err(Invalid);
        }
        let mut payload = Vec::new();
        payload.try_reserve_exact(length).map_err(|_| Unavailable)?;
        payload.resize(length, 0);
        file.read_exact(&mut payload).map_err(read_error)?;
        let digest = frame_digest(magic, &payload);
        if header[20..52] != digest {
            return Err(Invalid);
        }
        let after = file.metadata().map_err(|_| Unavailable)?;
        let named_after = rustix::fs::statat(self.root()?, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Invalid)?;
        if !same_file(&opened, &after)
            || !stat_matches(&named_after, &after)
            || opened.len() != after.len()
        {
            return Err(Invalid);
        }
        self.verify_lineage()?;
        Ok((payload, digest, opened.len()))
    }

    fn write_frame(
        &self,
        name: &str,
        magic: [u8; 16],
        payload: &[u8],
        max_payload: usize,
    ) -> Result<[u8; 32], MusubiPinIntentOutboxErrorV1> {
        use MusubiPinIntentOutboxErrorV1::{Capacity, Conflict, Invalid, Unavailable};
        if payload.is_empty() || payload.len() > max_payload {
            return Err(Capacity);
        }
        let length = u32::try_from(payload.len()).map_err(|_| Capacity)?;
        self.verify_lineage()?;
        match rustix::fs::statat(self.root()?, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => return Err(Conflict),
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(Unavailable),
        }
        let pending = format!("{name}.pending");
        let mut file = File::from(
            rustix::fs::openat(
                self.root()?,
                pending.as_str(),
                OFlags::WRONLY
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            )
            .map_err(|error| {
                if error == rustix::io::Errno::EXIST {
                    Invalid
                } else {
                    Unavailable
                }
            })?,
        );
        let digest = frame_digest(magic, payload);
        let mut header = [0_u8; FRAME_HEADER_BYTES];
        header[..16].copy_from_slice(&magic);
        header[16..20].copy_from_slice(&length.to_be_bytes());
        header[20..52].copy_from_slice(&digest);
        file.write_all(&header)
            .and_then(|()| file.write_all(payload))
            .and_then(|()| file.set_permissions(fs::Permissions::from_mode(0o400)))
            .and_then(|()| file.sync_all())
            .map_err(|_| Unavailable)?;
        drop(file);
        self.verify_lineage()?;
        rustix::fs::renameat_with(
            self.root()?,
            pending.as_str(),
            self.root()?,
            name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| {
            if error == rustix::io::Errno::EXIST {
                Conflict
            } else {
                Unavailable
            }
        })?;
        self.root()?.sync_all().map_err(|_| Unavailable)?;
        let (installed, installed_digest, _) = self.read_frame(name, magic, max_payload)?;
        if installed != payload || installed_digest != digest {
            return Err(Invalid);
        }
        Ok(digest)
    }
}

fn frame_digest(magic: [u8; 16], payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:pin-intent-frame:v1");
    hasher.update(&magic);
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}
fn read_error(error: std::io::Error) -> MusubiPinIntentOutboxErrorV1 {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        MusubiPinIntentOutboxErrorV1::Invalid
    } else {
        MusubiPinIntentOutboxErrorV1::Unavailable
    }
}
fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
}
fn safe_ancestor(metadata: &Metadata, owner: u32) -> bool {
    metadata.is_dir()
        && (metadata.uid() == 0 || metadata.uid() == owner)
        && (metadata.mode() & 0o022 == 0 || (metadata.uid() == 0 && metadata.mode() & 0o1000 != 0))
}
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
}
fn stat_matches(stat: &Stat, metadata: &Metadata) -> bool {
    stat.st_dev as u64 == metadata.dev()
        && stat.st_ino as u64 == metadata.ino()
        && stat.st_mode as u32 == metadata.mode()
        && stat.st_uid == metadata.uid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musubi_publication_service::finality::tests::reader_fixture;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        isi::{InstructionBox, sorafs::RegisterPinManifest},
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::{
        DagCodecId, ManifestBuilder, PinPolicy, ProfileId, StorageClass as ManifestStorageClass,
    };
    use std::time::Duration;

    fn private_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("private pin-intent root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private pin-intent mode");
        root
    }

    fn signed_pin(
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> (
        MusubiPublicationPaidPinPolicy,
        ManifestDigest,
        SignedTransaction,
    ) {
        signed_pin_with_policy(source, ManifestStorageClass::Hot, 30 * 24 * 60 * 60, 3)
    }

    fn signed_pin_with_policy(
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        storage_class: ManifestStorageClass,
        retention_horizon_secs: u64,
        min_replicas: u16,
    ) -> (
        MusubiPublicationPaidPinPolicy,
        ManifestDigest,
        SignedTransaction,
    ) {
        let key =
            KeyPair::try_from_seed(vec![0x7a; 32], Algorithm::Ed25519).expect("pin authority key");
        let authority = AccountId::new(key.public_key().clone());
        let archive = &source.registration.commitment;
        let manifest = ManifestBuilder::new()
            .root_cid(archive.root_cid.as_bytes().to_vec())
            .dag_codec(DagCodecId(sorafs_manifest::MANIFEST_DAG_CODEC))
            .chunking_from_registry(ProfileId(1))
            .chunk_digest_sha3_256(*archive.chunk_plan_digest.as_bytes())
            .por_root(*archive.por_root.as_bytes())
            .content_length(archive.content_length)
            .car_digest(*archive.car_digest.as_bytes())
            .car_size(archive.car_size)
            .pin_policy(PinPolicy {
                min_replicas,
                storage_class,
                retention_epoch: 43
                    + DEFAULT_TRANSACTION_TIME_TO_LIVE.as_secs()
                    + retention_horizon_secs,
            })
            .build()
            .expect("valid pin manifest");
        let digest = ManifestDigest::from_manifest(&manifest).expect("canonical digest");
        let instruction = InstructionBox::from(RegisterPinManifest::new(
            manifest.encode().expect("canonical manifest"),
            None,
            None,
        ));
        let mut builder = TransactionBuilder::new(
            source.network_id,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(vec![instruction]);
        builder.set_creation_time(Duration::from_millis(42_001));
        let transaction = builder.sign(key.private_key());
        (
            MusubiPublicationPaidPinPolicy {
                storage_class: StorageClass::Hot,
                retention_horizon_secs: 30 * 24 * 60 * 60,
                transaction_authority: authority,
            },
            digest,
            transaction,
        )
    }

    #[test]
    fn signed_intent_survives_reopen_with_exact_wire_and_conflicts_close() {
        let fixture = reader_fixture();
        let (policy, digest, transaction) = signed_pin(&fixture.query);
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 2,
            max_total_bytes: 16 * 1024 * 1024,
        };
        assert_eq!(
            DurableMusubiPinIntentOutboxV1::initialize(
                &root,
                fixture.query.network_id,
                [0; 32],
                policy.clone(),
                limits,
            ),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize once");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open owner");
        assert_eq!(outbox.retained_digests().expect("empty inventory"), []);
        assert_eq!(outbox.session_id(), [0xa1; 32]);
        let empty_inventory = outbox.inventory_digest().expect("complete empty inventory");
        outbox
            .stage_signed_intent([0x61; 32], &fixture.query, digest, &transaction)
            .expect("signed wire durable before dispatch");
        let staged_inventory = outbox
            .inventory_digest()
            .expect("complete staged inventory");
        assert_ne!(staged_inventory, empty_inventory);
        let finalized = MusubiPinOutboxHighWaterV1 {
            version: 1,
            network_id: fixture.query.network_id,
            pin_authority: policy.transaction_authority.clone(),
            session_id: outbox.session_id(),
            revision: 2,
            inventory_digest: staged_inventory,
            recorded_at_height: 3,
            transaction_hash: [0xd1; 32],
        };
        outbox
            .verify_finalized_high_water(&finalized)
            .expect("exact finalized inventory matches");
        let mut rolled_back = finalized.clone();
        rolled_back.inventory_digest = empty_inventory;
        assert_eq!(
            outbox.verify_finalized_high_water(&rolled_back),
            Err(MusubiPinIntentOutboxErrorV1::Invalid)
        );
        let mut switched_session = finalized.clone();
        switched_session.session_id = [0xa2; 32];
        assert_eq!(
            outbox.verify_finalized_high_water(&switched_session),
            Err(MusubiPinIntentOutboxErrorV1::Invalid)
        );
        outbox
            .stage_signed_intent([0x61; 32], &fixture.query, digest, &transaction)
            .expect("exact retry idempotent");
        assert_eq!(
            outbox.stage_signed_intent([0x62; 32], &fixture.query, digest, &transaction),
            Err(MusubiPinIntentOutboxErrorV1::Conflict)
        );
        assert_eq!(outbox.retained_digests().expect("inventory"), [digest]);
        assert_eq!(
            outbox
                .recover_operation([0x61; 32])
                .expect("recover retained operation")
                .manifest_digest,
            digest,
        );
        assert_eq!(
            outbox.recover_operation([0x69; 32]).map(|_| ()),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
        assert!(matches!(
            outbox.recover_operation([0; 32]),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        ));
        let recovered = outbox
            .recover_signed_intent(digest)
            .expect("recover signed wire");
        assert_eq!(recovered.operation_id, [0x61; 32]);
        assert_eq!(recovered.source, fixture.query);
        assert_eq!(
            recovered
                .transaction
                .encode_wire_v1()
                .expect("recovered wire"),
            transaction.encode_wire_v1().expect("original wire")
        );
        drop(outbox);
        let reopened = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy,
            limits,
            fixture.reader,
        )
        .expect("restart reopens immutable owner");
        assert_eq!(
            reopened.retained_digests().expect("restart inventory"),
            [digest]
        );
        assert_eq!(
            reopened.inventory_digest().expect("reopened inventory"),
            staged_inventory
        );
        reopened
            .verify_finalized_high_water(&finalized)
            .expect("restarted inventory still matches finalized high-water");
        assert_eq!(
            reopened
                .recover_signed_intent(digest)
                .expect("restart recovered exact wire")
                .transaction
                .encode_wire_v1()
                .expect("wire"),
            transaction.encode_wire_v1().expect("wire")
        );
    }

    #[test]
    fn local_finality_audit_detects_missing_anchor_mutation_and_restart_rollback() {
        use MusubiPublicationPinOutboxHighWaterReadErrorV1::{Invalid, LocallyAhead};

        let fixture = reader_fixture();
        let (policy, digest, transaction) = signed_pin(&fixture.query);
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 1,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialized immutable owner");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open original custody");
        outbox
            .stage_signed_intent([0x61; 32], &fixture.query, digest, &transaction)
            .expect("persist exact signed wire");
        let record = MusubiPinOutboxHighWaterV1 {
            version: 1,
            network_id: fixture.query.network_id,
            pin_authority: policy.transaction_authority.clone(),
            session_id: outbox.session_id(),
            revision: 2,
            inventory_digest: outbox.inventory_digest().expect("complete inventory"),
            recorded_at_height: 3,
            transaction_hash: [0xd1; 32],
        };
        let expected = MusubiPinIntentOutboxLocalAuditV1 {
            session_id: record.session_id,
            revision: record.revision,
            inventory_digest: record.inventory_digest,
            retained_records: 1,
        };
        assert_eq!(
            audit_finalized_inventory_with(&outbox, |authority| {
                assert_eq!(authority, &policy.transaction_authority);
                Ok(Some(record.clone()))
            }),
            Ok(expected),
        );
        assert_eq!(
            audit_finalized_inventory_with(&outbox, |_| Ok(None)),
            Err(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor),
        );
        assert_eq!(
            audit_finalized_inventory_with(&outbox, |_| Err(LocallyAhead)),
            Err(MusubiPinIntentOutboxErrorV1::LocallyAhead),
        );
        assert_eq!(
            audit_finalized_inventory_with(&outbox, |_| Err(Invalid)),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
        let mut changed = record.clone();
        changed.session_id = [0xa2; 32];
        assert_eq!(
            audit_finalized_inventory_with(&outbox, |_| Ok(Some(changed))),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
        drop(outbox);
        let reopened = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("restart under same owner");
        assert_eq!(
            audit_finalized_inventory_with(&reopened, |_| Ok(Some(record.clone()))),
            Ok(expected),
        );
        drop(reopened);
        fs::remove_file(root.join(intent_name(digest)))
            .expect("simulate offline directory rollback");
        let rolled_back = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy,
            limits,
            fixture.reader,
        )
        .expect("older owner-only directory is locally well-formed");
        assert_eq!(
            audit_finalized_inventory_with(&rolled_back, |_| Ok(Some(record))),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
    }

    #[test]
    fn restored_older_inventory_cannot_recover_an_expected_signed_operation() {
        let fixture = reader_fixture();
        let (policy, digest, transaction) = signed_pin(&fixture.query);
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 1,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize owner-only directory");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open outbox");
        outbox
            .stage_signed_intent([0x68; 32], &fixture.query, digest, &transaction)
            .expect("durably stage exact signed transaction");
        let finalized = MusubiPinOutboxHighWaterV1 {
            version: 1,
            network_id: fixture.query.network_id,
            pin_authority: policy.transaction_authority.clone(),
            session_id: outbox.session_id(),
            revision: 2,
            inventory_digest: outbox.inventory_digest().expect("staged inventory digest"),
            recorded_at_height: 3,
            transaction_hash: [0xd1; 32],
        };
        drop(outbox);

        // A privileged offline restore of an earlier owner-only inventory remains locally valid,
        // but the separately finalized complete inventory digest exposes the rollback.
        fs::remove_file(root.join(intent_name(digest))).expect("simulate offline restore");
        let reopened = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy,
            limits,
            fixture.reader,
        )
        .expect("owner-only inventory is locally well-formed");
        assert_eq!(reopened.retained_digests().expect("older inventory"), []);
        assert_eq!(
            reopened.verify_finalized_high_water(&finalized),
            Err(MusubiPinIntentOutboxErrorV1::Invalid)
        );
        assert_eq!(
            reopened.recover_operation([0x68; 32]).map(|_| ()),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
    }

    #[test]
    fn missing_owner_pending_and_online_substitution_close_recovery() {
        let fixture = reader_fixture();
        let (policy, digest, transaction) = signed_pin(&fixture.query);
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 1,
            max_total_bytes: 16 * 1024 * 1024,
        };
        assert!(
            DurableMusubiPinIntentOutboxV1::open_inner(
                &root,
                fixture.query.network_id,
                policy.clone(),
                limits,
                fixture.reader.clone(),
            )
            .is_err()
        );
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize once");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open owner");
        outbox
            .stage_signed_intent([0x63; 32], &fixture.query, digest, &transaction)
            .expect("stage signed wire");
        fs::write(
            root.join(format!("{}.pending", intent_name(digest))),
            b"interrupted",
        )
        .expect("interrupted write marker");
        assert_eq!(
            outbox.retained_digests(),
            Err(MusubiPinIntentOutboxErrorV1::Invalid)
        );
        drop(outbox);
        assert!(
            DurableMusubiPinIntentOutboxV1::open_inner(
                &root,
                fixture.query.network_id,
                policy,
                limits,
                fixture.reader,
            )
            .is_err()
        );
    }

    #[test]
    fn exact_paid_pin_policy_controls_stage_and_reopen() {
        let fixture = reader_fixture();
        let (policy, _, _) = signed_pin(&fixture.query);
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 2,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize exact policy");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open exact policy");
        for (tier, horizon, replicas) in [
            (ManifestStorageClass::Warm, policy.retention_horizon_secs, 3),
            (
                ManifestStorageClass::Hot,
                policy.retention_horizon_secs + 1,
                3,
            ),
            (ManifestStorageClass::Hot, policy.retention_horizon_secs, 2),
        ] {
            let (_, digest, transaction) =
                signed_pin_with_policy(&fixture.query, tier, horizon, replicas);
            assert_eq!(
                outbox.stage_signed_intent([0x71; 32], &fixture.query, digest, &transaction),
                Err(MusubiPinIntentOutboxErrorV1::Invalid),
            );
        }
        drop(outbox);
        let mut wrong_tier = policy.clone();
        wrong_tier.storage_class = StorageClass::Cold;
        let mut wrong_horizon = policy.clone();
        wrong_horizon.retention_horizon_secs += 1;
        let mut wrong_authority = policy.clone();
        wrong_authority.transaction_authority = AccountId::new(
            KeyPair::try_from_seed(vec![0x7b; 32], Algorithm::Ed25519)
                .expect("other pin key")
                .public_key()
                .clone(),
        );
        for wrong in [wrong_tier, wrong_horizon, wrong_authority] {
            assert!(matches!(
                DurableMusubiPinIntentOutboxV1::open_inner(
                    &root,
                    fixture.query.network_id,
                    wrong,
                    limits,
                    fixture.reader.clone(),
                ),
                Err(MusubiPinIntentOutboxErrorV1::Invalid),
            ));
        }
        let mut impossible_horizon = policy;
        impossible_horizon.retention_horizon_secs =
            u64::from(SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1);
        assert_eq!(
            DurableMusubiPinIntentOutboxV1::initialize(
                &root,
                fixture.query.network_id,
                [0xa1; 32],
                impossible_horizon,
                limits,
            ),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
    }

    #[test]
    fn selected_warm_tier_and_horizon_survive_recovery() {
        let fixture = reader_fixture();
        let (mut policy, digest, transaction) = signed_pin_with_policy(
            &fixture.query,
            ManifestStorageClass::Warm,
            90 * 24 * 60 * 60,
            4,
        );
        policy.storage_class = StorageClass::Warm;
        policy.retention_horizon_secs = 90 * 24 * 60 * 60;
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 1,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize configured warm policy");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy.clone(),
            limits,
            fixture.reader.clone(),
        )
        .expect("open warm outbox");
        outbox
            .stage_signed_intent([0x73; 32], &fixture.query, digest, &transaction)
            .expect("stage exact warm paid pin");
        drop(outbox);
        let reopened = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            fixture.query.network_id,
            policy,
            limits,
            fixture.reader,
        )
        .expect("recover warm outbox");
        assert_eq!(
            reopened
                .recover_signed_intent(digest)
                .expect("recover warm intent")
                .transaction
                .encode_wire_v1()
                .expect("recovered wire"),
            transaction.encode_wire_v1().expect("original wire"),
        );
    }

    #[test]
    fn reframed_retained_policy_record_and_online_owner_tamper_close_recovery() {
        let fixture = reader_fixture();
        let (policy, digest, transaction) = signed_pin_with_policy(
            &fixture.query,
            ManifestStorageClass::Warm,
            30 * 24 * 60 * 60,
            3,
        );
        let temp = private_root();
        let root = temp.path().canonicalize().expect("canonical private root");
        let limits = MusubiPinIntentOutboxLimitsV1 {
            max_records: 2,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize exact policy");
        let record = StoredSignedPinIntentV1 {
            version: 1,
            operation_id: [0x72; 32],
            source: fixture.query.clone(),
            manifest_digest: digest,
            signed_wire: transaction.encode_wire_v1().expect("signed wire"),
        };
        let bytes = norito::encode_canonical(&record).expect("canonical retained record");
        PinnedPrivateRootV1::open(&root)
            .expect("pin private directory")
            .write_frame(&intent_name(digest), INTENT_MAGIC, &bytes, MAX_INTENT_BYTES)
            .expect("durable, well-framed but wrong-tier record");
        assert!(matches!(
            DurableMusubiPinIntentOutboxV1::open_inner(
                &root,
                fixture.query.network_id,
                policy,
                limits,
                fixture.reader.clone(),
            ),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        ));

        let clean = private_root();
        let clean_root = clean.path().canonicalize().expect("clean canonical root");
        let (policy, _, _) = signed_pin(&fixture.query);
        DurableMusubiPinIntentOutboxV1::initialize(
            &clean_root,
            fixture.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize clean owner");
        let outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &clean_root,
            fixture.query.network_id,
            policy,
            limits,
            fixture.reader,
        )
        .expect("open clean owner");
        let owner_path = clean_root.join(OWNER_FILE);
        fs::set_permissions(&owner_path, fs::Permissions::from_mode(0o600))
            .expect("offline owner-mode tamper");
        fs::write(owner_path, b"substituted owner policy").expect("mutate retained owner in place");
        assert_eq!(
            outbox.retained_digests(),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
    }
}
