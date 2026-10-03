//! Durable, separate seed custody for the already verified private Musubi ingress route.
//!
//! An issued receipt does not make this CAR a finalized provider pin. This owner retains its
//! exact bytes until a caller presents a registration authenticated by the daemon's finalized
//! State/Kura reader. Receipt expiry alone never removes staged bytes.
use iroha_data_model::{
    musubi::{
        MUSUBI_MAX_CAR_BYTES_V1, MusubiArchiveCommitmentV1, MusubiArchiveRecordV1,
        MusubiSeedIngressReceiptBindingV1,
    },
    sorafs::capacity::ProviderId,
};
use iroha_musubi_service::{
    MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1, MusubiPublicationServiceBackendErrorV1,
    MusubiSeedIngressBackendV1, MusubiSeedIngressCarPlanV1,
};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use sorafs_car::{CarBuildPlan, musubi::MusubiBundleVerifierV1};
use std::{
    ffi::OsString,
    fs::{self, File, Metadata},
    io::{Read as _, Write as _},
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Component, Path},
};

const STAGE_MAGIC: [u8; 16] = *b"musubi-seed-v1\0\0";
const STAGE_HEADER_BYTES: usize = 32;
const MAX_STAGE_METADATA_BYTES: usize = 64 * 1024;
const MAX_STAGE_RECORDS: u32 = 1_024;
const MAX_STAGE_RECORD_BYTES: u64 = STAGE_HEADER_BYTES as u64
    + MAX_STAGE_METADATA_BYTES as u64
    + MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1 as u64
    + MUSUBI_MAX_CAR_BYTES_V1;
const STAGE_SUFFIX: &str = ".seed";

/// Closed failure for the private seed-custody directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusubiSeedStagingErrorV1 {
    /// Original finalized-history allocation refusal, retained for a local retry.
    Deferred(iroha_core::execution_attempt::ExecutionDeferred),
    /// The configured root, its ancestry, or a resident record is unsafe or substituted.
    Invalid,
    /// Another process owns the staging directory.
    Locked,
    /// An exact operation conflicts with immutable resident bytes or identity.
    Conflict,
    /// The fixed aggregate retention limit is exhausted.
    Capacity,
    /// Storage or local allocation is temporarily unavailable.
    Unavailable,
    /// The requested historical archive has not finalized in this node's current view.
    LocallyAhead,
}
impl MusubiSeedStagingErrorV1 {
    fn backend(self) -> MusubiPublicationServiceBackendErrorV1 {
        match self {
            Self::Deferred(_)
            | Self::Locked
            | Self::Capacity
            | Self::Unavailable
            | Self::LocallyAhead => MusubiPublicationServiceBackendErrorV1::Retryable,
            Self::Invalid | Self::Conflict => MusubiPublicationServiceBackendErrorV1::Permanent,
        }
    }
}

#[derive(
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
    Clone,
    Debug,
    PartialEq,
    Eq,
)]
#[norito_schema(name = "irohad::musubi_publication_service::StagedSeedMetadataV1")]
struct StagedSeedMetadataV1 {
    version: u8,
    operation_id: [u8; 32],
    binding: MusubiSeedIngressReceiptBindingV1,
    commitment: MusubiArchiveCommitmentV1,
}
struct PinnedDirectoryV1 {
    name: OsString,
    file: File,
    metadata: Metadata,
}
struct StagedSeedRecordV1 {
    metadata: StagedSeedMetadataV1,
    plan: CarBuildPlan,
    car: Vec<u8>,
}

/// A bounded, process-exclusive seed backend for one exact provider and deployment root.
///
/// The root must already exist with mode 0700 and be owned by the daemon UID. Every ancestor is
/// retained as a descriptor and rechecked before use. Records are installed without replacement
/// as immutable single-link mode-0400 files. An interrupted write leaves a visible tombstone that
/// blocks new staging until an operator repairs the directory; this owner never deletes on expiry.
pub struct MusubiSeedStagingBackendV1 {
    provider: ProviderId,
    lineage: Vec<PinnedDirectoryV1>,
    owner_uid: u32,
    max_records: u32,
    max_total_bytes: u64,
}
impl std::fmt::Debug for MusubiSeedStagingBackendV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MusubiSeedStagingBackendV1")
            .field("provider", &self.provider)
            .field("max_records", &self.max_records)
            .field("max_total_bytes", &self.max_total_bytes)
            .finish_non_exhaustive()
    }
}
impl MusubiSeedStagingBackendV1 {
    /// Pin an existing owner-only directory and its immutable retention limits.
    ///
    /// # Errors
    /// Rejects linked or writable ancestry, an unsafe root, a competing process, invalid limits,
    /// unsafe resident entry names or metadata, or inventory beyond the configured ceilings.
    pub fn open(
        root: &Path,
        provider: ProviderId,
        max_records: u32,
        max_total_bytes: u64,
    ) -> Result<Self, MusubiSeedStagingErrorV1> {
        if !root.is_absolute()
            || max_records == 0
            || max_records > MAX_STAGE_RECORDS
            || max_total_bytes == 0
            || max_total_bytes > MAX_STAGE_RECORD_BYTES * u64::from(MAX_STAGE_RECORDS)
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let owner_uid = rustix::process::geteuid().as_raw();
        let root_file = File::from(
            rustix::fs::open("/", directory_flags(), Mode::empty())
                .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?,
        );
        let root_metadata = root_file
            .metadata()
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        let mut lineage = vec![PinnedDirectoryV1 {
            name: OsString::from("/"),
            file: root_file,
            metadata: root_metadata,
        }];
        let mut reconstructed = std::path::PathBuf::from("/");
        for component in root.components() {
            let Component::Normal(name) = component else {
                if component == Component::RootDir {
                    continue;
                }
                return Err(MusubiSeedStagingErrorV1::Invalid);
            };
            reconstructed.push(name);
            let parent = &lineage
                .last()
                .ok_or(MusubiSeedStagingErrorV1::Invalid)?
                .file;
            let before = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
            let file = File::from(
                rustix::fs::openat(parent, name, directory_flags(), Mode::empty())
                    .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?,
            );
            let metadata = file
                .metadata()
                .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
            if !safe_ancestor(&metadata, owner_uid) || !stat_matches(&before, &metadata) {
                return Err(MusubiSeedStagingErrorV1::Invalid);
            }
            lineage.push(PinnedDirectoryV1 {
                name: name.to_owned(),
                file,
                metadata,
            });
        }
        if reconstructed != root || lineage.len() < 2 {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let leaf = lineage.last().ok_or(MusubiSeedStagingErrorV1::Invalid)?;
        if leaf.metadata.uid() != owner_uid || leaf.metadata.mode() & 0o7777 != 0o700 {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        rustix::fs::flock(
            &leaf.file,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(|error| {
            if error == rustix::io::Errno::WOULDBLOCK {
                MusubiSeedStagingErrorV1::Locked
            } else {
                MusubiSeedStagingErrorV1::Unavailable
            }
        })?;
        let backend = Self {
            provider,
            lineage,
            owner_uid,
            max_records,
            max_total_bytes,
        };
        let (records, bytes) = backend.inventory()?;
        if records > max_records || bytes > max_total_bytes {
            return Err(MusubiSeedStagingErrorV1::Capacity);
        }
        Ok(backend)
    }

    /// Recover one exact staged record after the caller authenticated its archive with State/Kura.
    ///
    /// Only the daemon-owned finalized seed capability calls this internal method. The record is
    /// bounded before allocation and fully reverified before bytes leave local custody.
    pub(super) fn read_verified_archive_seed(
        &self,
        archive: &MusubiArchiveRecordV1,
    ) -> Result<(CarBuildPlan, Vec<u8>), MusubiSeedStagingErrorV1> {
        let receipt = &archive.staging_receipt;
        let binding = &receipt.payload.binding;
        receipt
            .verify(binding, receipt.payload.issued_at_ms)
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        if binding.seed_provider != self.provider
            || archive.commitment.archive_id() != binding.archive_id
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let key = record_name(binding)?;
        let record = self.read_record(&key)?;
        if record.metadata.binding != *binding || record.metadata.commitment != archive.commitment {
            return Err(MusubiSeedStagingErrorV1::Conflict);
        }
        Ok((record.plan, record.car))
    }

    fn root(&self) -> Result<&File, MusubiSeedStagingErrorV1> {
        self.lineage
            .last()
            .map(|directory| &directory.file)
            .ok_or(MusubiSeedStagingErrorV1::Invalid)
    }
    fn verify_lineage(&self) -> Result<(), MusubiSeedStagingErrorV1> {
        for (index, directory) in self.lineage.iter().enumerate() {
            let current = directory
                .file
                .metadata()
                .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
            if !safe_ancestor(&current, self.owner_uid) || !same_file(&current, &directory.metadata)
            {
                return Err(MusubiSeedStagingErrorV1::Invalid);
            }
            if index > 0 {
                let named = rustix::fs::statat(
                    &self.lineage[index - 1].file,
                    &directory.name,
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
                if !stat_matches(&named, &current) {
                    return Err(MusubiSeedStagingErrorV1::Invalid);
                }
            }
        }
        Ok(())
    }
    fn inventory(&self) -> Result<(u32, u64), MusubiSeedStagingErrorV1> {
        self.verify_lineage()?;
        let entries = rustix::fs::Dir::read_from(self.root()?)
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        let mut records = 0u32;
        let mut bytes = 0u64;
        for entry in entries {
            let entry = entry.map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let text = std::str::from_utf8(name.to_bytes())
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
            let Some(hex_key) = text.strip_suffix(STAGE_SUFFIX) else {
                // Interrupted writes are visible tombstones; never silently erase them.
                return Err(MusubiSeedStagingErrorV1::Invalid);
            };
            if hex_key.len() != 64
                || !hex_key
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err(MusubiSeedStagingErrorV1::Invalid);
            }
            let stat = rustix::fs::statat(self.root()?, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
                || stat.st_nlink != 1
                || stat.st_uid != self.owner_uid
                || stat.st_mode & 0o7777 != 0o400
                || stat.st_size < STAGE_HEADER_BYTES as i64
                || stat.st_size as u64 > MAX_STAGE_RECORD_BYTES
            {
                return Err(MusubiSeedStagingErrorV1::Invalid);
            }
            records = records
                .checked_add(1)
                .ok_or(MusubiSeedStagingErrorV1::Capacity)?;
            bytes = bytes
                .checked_add(stat.st_size as u64)
                .ok_or(MusubiSeedStagingErrorV1::Capacity)?;
            if records > self.max_records || bytes > self.max_total_bytes {
                return Err(MusubiSeedStagingErrorV1::Capacity);
            }
        }
        Ok((records, bytes))
    }

    fn read_record(&self, name: &str) -> Result<StagedSeedRecordV1, MusubiSeedStagingErrorV1> {
        self.verify_lineage()?;
        let before =
            rustix::fs::statat(self.root()?, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                if error == rustix::io::Errno::NOENT {
                    MusubiSeedStagingErrorV1::Conflict
                } else {
                    MusubiSeedStagingErrorV1::Unavailable
                }
            })?;
        let mut file = File::from(
            rustix::fs::openat(
                self.root()?,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?,
        );
        let opened = file
            .metadata()
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        if !stat_matches(&before, &opened)
            || !opened.is_file()
            || opened.nlink() != 1
            || opened.uid() != self.owner_uid
            || opened.mode() & 0o7777 != 0o400
            || opened.len() > MAX_STAGE_RECORD_BYTES
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let mut header = [0u8; STAGE_HEADER_BYTES];
        file.read_exact(&mut header).map_err(read_error)?;
        if header[..16] != STAGE_MAGIC {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let metadata_len = u32::from_be_bytes(header[16..20].try_into().expect("fixed header"));
        let plan_len = u32::from_be_bytes(header[20..24].try_into().expect("fixed header"));
        let car_len = u64::from_be_bytes(header[24..32].try_into().expect("fixed header"));
        if metadata_len == 0
            || metadata_len as usize > MAX_STAGE_METADATA_BYTES
            || plan_len == 0
            || plan_len as usize > MUSUBI_MAX_SEED_INGRESS_PLAN_BYTES_V1
            || car_len == 0
            || car_len > MUSUBI_MAX_CAR_BYTES_V1
            || STAGE_HEADER_BYTES as u64 + u64::from(metadata_len) + u64::from(plan_len) + car_len
                != opened.len()
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let mut metadata_bytes = bounded_buffer(metadata_len as usize)?;
        let mut plan_bytes = bounded_buffer(plan_len as usize)?;
        let mut car = bounded_buffer(
            usize::try_from(car_len).map_err(|_| MusubiSeedStagingErrorV1::Invalid)?,
        )?;
        file.read_exact(&mut metadata_bytes)
            .and_then(|()| file.read_exact(&mut plan_bytes))
            .and_then(|()| file.read_exact(&mut car))
            .map_err(read_error)?;
        let after = file
            .metadata()
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        if !same_file(&after, &opened) || after.len() != opened.len() {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        self.verify_lineage()?;
        let metadata: StagedSeedMetadataV1 = norito::decode_canonical_with_limits(
            &metadata_bytes,
            norito::canonical_decode_limits(metadata_bytes.len()),
        )
        .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        if metadata.version != 1
            || metadata.operation_id == [0; 32]
            || metadata.binding.validate().is_err()
            || metadata.commitment.validate().is_err()
            || metadata.binding.archive_id != metadata.commitment.archive_id()
            || metadata.binding.seed_provider != self.provider
            || metadata.binding.car_body_digest != metadata.commitment.car_digest
            || metadata.binding.car_body_length != metadata.commitment.car_size
            || record_name(&metadata.binding)? != name
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let witness: MusubiSeedIngressCarPlanV1 = norito::decode_canonical_with_limits(
            &plan_bytes,
            norito::canonical_decode_limits(plan_bytes.len()),
        )
        .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        if witness
            .canonical_bytes()
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?
            != plan_bytes
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let plan = witness
            .to_car_build_plan(&metadata.commitment)
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        verify_exact_bundle(&metadata.binding, &metadata.commitment, &plan, &car)?;
        Ok(StagedSeedRecordV1 {
            metadata,
            plan,
            car,
        })
    }
}
impl MusubiSeedIngressBackendV1 for MusubiSeedStagingBackendV1 {
    fn provider_id(&self) -> ProviderId {
        self.provider
    }
    fn stage_exact_car(
        &mut self,
        operation_id: [u8; 32],
        binding: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
        self.stage(operation_id, binding, commitment, plan, car)
            .map_err(MusubiSeedStagingErrorV1::backend)
    }
    fn verify_staged_car(
        &self,
        operation_id: [u8; 32],
        binding: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<(), MusubiPublicationServiceBackendErrorV1> {
        let name = record_name(binding).map_err(MusubiSeedStagingErrorV1::backend)?;
        let retained = self
            .read_record(&name)
            .map_err(MusubiSeedStagingErrorV1::backend)?;
        if retained.metadata.operation_id != operation_id
            || retained.metadata.binding != *binding
            || retained.metadata.commitment != *commitment
            || retained.plan != *plan
            || retained.car != car
        {
            return Err(MusubiPublicationServiceBackendErrorV1::Permanent);
        }
        Ok(())
    }
}
impl MusubiSeedStagingBackendV1 {
    fn stage(
        &mut self,
        operation_id: [u8; 32],
        binding: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<(), MusubiSeedStagingErrorV1> {
        if operation_id == [0; 32]
            || binding.seed_provider != self.provider
            || binding.validate().is_err()
            || commitment.validate().is_err()
            || binding.archive_id != commitment.archive_id()
            || binding.car_body_digest != commitment.car_digest
            || binding.car_body_length != commitment.car_size
        {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        verify_exact_bundle(binding, commitment, plan, car)?;
        let witness = MusubiSeedIngressCarPlanV1::from_car_build_plan(plan, commitment)
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        let plan_bytes = witness
            .canonical_bytes()
            .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        let metadata = StagedSeedMetadataV1 {
            version: 1,
            operation_id,
            binding: binding.clone(),
            commitment: commitment.clone(),
        };
        let metadata_bytes =
            norito::encode_canonical(&metadata).map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
        if metadata_bytes.is_empty() || metadata_bytes.len() > MAX_STAGE_METADATA_BYTES {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        let name = record_name(binding)?;
        self.verify_lineage()?;
        match rustix::fs::statat(self.root()?, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => {
                let existing = self.read_record(&name)?;
                if existing.metadata == metadata && existing.plan == *plan && existing.car == car {
                    return Ok(());
                }
                return Err(MusubiSeedStagingErrorV1::Conflict);
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(MusubiSeedStagingErrorV1::Unavailable),
        }
        let (count, occupied) = self.inventory()?;
        let new_bytes = STAGE_HEADER_BYTES as u64
            + metadata_bytes.len() as u64
            + plan_bytes.len() as u64
            + car.len() as u64;
        if count >= self.max_records
            || occupied
                .checked_add(new_bytes)
                .is_none_or(|total| total > self.max_total_bytes)
        {
            return Err(MusubiSeedStagingErrorV1::Capacity);
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
                    MusubiSeedStagingErrorV1::Invalid
                } else {
                    MusubiSeedStagingErrorV1::Unavailable
                }
            })?,
        );
        let mut header = [0u8; STAGE_HEADER_BYTES];
        header[..16].copy_from_slice(&STAGE_MAGIC);
        header[16..20].copy_from_slice(
            &u32::try_from(metadata_bytes.len())
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?
                .to_be_bytes(),
        );
        header[20..24].copy_from_slice(
            &u32::try_from(plan_bytes.len())
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?
                .to_be_bytes(),
        );
        header[24..32].copy_from_slice(
            &u64::try_from(car.len())
                .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?
                .to_be_bytes(),
        );
        file.write_all(&header)
            .and_then(|()| file.write_all(&metadata_bytes))
            .and_then(|()| file.write_all(&plan_bytes))
            .and_then(|()| file.write_all(car))
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        file.set_permissions(fs::Permissions::from_mode(0o400))
            .and_then(|()| file.sync_all())
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        drop(file);
        rustix::fs::renameat_with(
            self.root()?,
            pending.as_str(),
            self.root()?,
            name.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| {
            if error == rustix::io::Errno::EXIST {
                MusubiSeedStagingErrorV1::Conflict
            } else {
                MusubiSeedStagingErrorV1::Unavailable
            }
        })?;
        self.root()?
            .sync_all()
            .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
        let installed = self.read_record(&name)?;
        if installed.metadata != metadata || installed.plan != *plan || installed.car != car {
            return Err(MusubiSeedStagingErrorV1::Invalid);
        }
        Ok(())
    }
}

fn verify_exact_bundle(
    binding: &MusubiSeedIngressReceiptBindingV1,
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
    car: &[u8],
) -> Result<(), MusubiSeedStagingErrorV1> {
    if u64::try_from(car.len()).ok() != Some(commitment.car_size)
        || blake3::hash(car).as_bytes() != commitment.car_digest.as_bytes()
    {
        return Err(MusubiSeedStagingErrorV1::Invalid);
    }
    let verified = MusubiBundleVerifierV1::verify(plan, car, commitment)
        .map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
    if verified.semantic_release().semantic_digest() != binding.semantic_release_manifest_digest {
        return Err(MusubiSeedStagingErrorV1::Invalid);
    }
    Ok(())
}
fn record_name(
    binding: &MusubiSeedIngressReceiptBindingV1,
) -> Result<String, MusubiSeedStagingErrorV1> {
    let encoded =
        norito::encode_canonical(binding).map_err(|_| MusubiSeedStagingErrorV1::Invalid)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"iroha.musubi.seed-staging-key.v1");
    hasher.update(&encoded);
    Ok(format!(
        "{}{}",
        hex::encode(hasher.finalize().as_bytes()),
        STAGE_SUFFIX
    ))
}
fn bounded_buffer(length: usize) -> Result<Vec<u8>, MusubiSeedStagingErrorV1> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(length)
        .map_err(|_| MusubiSeedStagingErrorV1::Unavailable)?;
    buffer.resize(length, 0);
    Ok(buffer)
}
fn read_error(error: std::io::Error) -> MusubiSeedStagingErrorV1 {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        MusubiSeedStagingErrorV1::Invalid
    } else {
        MusubiSeedStagingErrorV1::Unavailable
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
/// Canonical bundle fixtures shared by the daemon's private-publication adapter tests.
pub(crate) mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        block::BlockHeader,
        musubi::{
            MusubiAbiBindingV1, MusubiArtifactDescriptorV1, MusubiContentDigestV1,
            MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiReleaseIdV1,
            MusubiReleaseMetadataV1, MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1,
        },
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
    };
    use iroha_model_base::topology::DataSpaceId;
    use norito::codec::Encode as _;
    use sorafs_car::{
        CarWriter, FileEntry, compute_chunk_plan_digest_sha3, compute_por_root,
        musubi::{
            MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1, MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1,
            MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1,
        },
    };

    fn frame(output: &mut Vec<u8>, bytes: &[u8]) {
        output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        output.extend_from_slice(bytes);
    }
    fn digest(domain: &[u8], material: &[u8]) -> MusubiContentDigestV1 {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher.update(&(material.len() as u64).to_be_bytes());
        hasher.update(material);
        MusubiContentDigestV1::new(*hasher.finalize().as_bytes())
    }
    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("test signing key")
    }
    fn path(value: &str) -> Vec<String> {
        value.split('/').map(str::to_owned).collect()
    }
    #[allow(
        clippy::too_many_lines,
        reason = "one complete canonical Musubi bundle fixture"
    )]
    /// Build one exact bundle, receipt binding, and canonical CAR for adapter tests.
    pub(crate) fn fixture() -> (
        MusubiSeedIngressReceiptBindingV1,
        MusubiArchiveCommitmentV1,
        CarBuildPlan,
        Vec<u8>,
    ) {
        let release = MusubiReleaseIdV1::new(
            MusubiPackageIdV1::new(
                DataSpaceId::new(7),
                MusubiPackageScopeV1::DataspaceRoot,
                "seed-stage".parse().expect("package name"),
            ),
            "1.0.0".parse().expect("package version"),
        );
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: 1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let semantic = MusubiSemanticReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0x71; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0x72; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            verification_lock_digest: lock.digest(),
        };
        let source_path = "Musubi.toml";
        let source_data = b"[package]\nname='seed-stage'\n".to_vec();
        let mut source_material = Vec::new();
        frame(&mut source_material, b"musubi-source-tree-v1\0");
        source_material.extend_from_slice(&1u32.to_be_bytes());
        frame(&mut source_material, source_path.as_bytes());
        source_material.extend_from_slice(&(source_data.len() as u64).to_be_bytes());
        source_material.extend_from_slice(blake3::hash(&source_data).as_bytes());
        let source_digest = digest(b"musubi-source-tree-v1\0", &source_material);
        let descriptor = MusubiArtifactDescriptorV1::new(
            semantic.semantic_digest(),
            source_digest,
            lock.digest(),
            source_data.len() as u64,
            1,
        )
        .expect("descriptor");
        let semantic_bytes = semantic.encode();
        let descriptor_bytes = descriptor.encode();
        let lock_bytes = lock.encode();
        let mut descriptor_material = Vec::new();
        frame(&mut descriptor_material, b"musubi-artifact-descriptor-v1\0");
        frame(&mut descriptor_material, &descriptor_bytes);
        let descriptor_digest = digest(b"musubi-artifact-descriptor-v1\0", &descriptor_material);
        let mut bundle_material = Vec::new();
        for bytes in [
            b"musubi-bundle-v1\0".as_slice(),
            semantic_bytes.as_slice(),
            descriptor_material.as_slice(),
            source_material.as_slice(),
            lock_bytes.as_slice(),
        ] {
            frame(&mut bundle_material, bytes);
        }
        let bundle_digest = digest(b"musubi-bundle-v1\0", &bundle_material);
        let entries = vec![
            FileEntry {
                path: path(source_path),
                data: source_data,
            },
            FileEntry {
                path: path(MUSUBI_BUNDLE_SEMANTIC_RELEASE_PATH_V1),
                data: semantic_bytes,
            },
            FileEntry {
                path: path(MUSUBI_BUNDLE_ARTIFACT_DESCRIPTOR_PATH_V1),
                data: descriptor_bytes,
            },
            FileEntry {
                path: path(MUSUBI_BUNDLE_VERIFICATION_LOCK_PATH_V1),
                data: lock_bytes,
            },
        ];
        let (plan, payload) = CarBuildPlan::from_files(entries).expect("plan");
        let mut car = Vec::new();
        let stats = CarWriter::new(&plan, &payload)
            .expect("CAR writer")
            .write_to(&mut car)
            .expect("canonical CAR");
        let registered = sorafs_car::chunker_registry::default_descriptor();
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::try_from(stats.root_cids[0].clone())
                .expect("canonical root"),
            chunker: ChunkerProfileHandle {
                profile_id: registered.id.0,
                namespace: registered.namespace.to_owned(),
                name: registered.name.to_owned(),
                semver: registered.semver.to_owned(),
                multihash_code: registered.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(
                &plan.chunks,
            )),
            por_root: MusubiContentDigestV1::new(compute_por_root(&payload, &plan).expect("PoR")),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
            car_size: stats.car_size,
            bundle_digest,
            source_tree_digest: source_digest,
            descriptor_digest,
            file_count: 1,
            chunk_count: plan.chunks.len() as u32,
        };
        let publisher = AccountId::new(key(0x31).public_key().clone());
        let broker = AccountId::new(key(0x32).public_key().clone());
        let binding = MusubiSeedIngressReceiptBindingV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([0x15; 32])),
            ),
            publisher,
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x33; 32]),
            semantic_release_manifest_digest: semantic.semantic_digest(),
            archive_id: commitment.archive_id(),
            car_body_digest: commitment.car_digest,
            car_body_length: commitment.car_size,
            nonce: [0x34; 32],
        };
        (binding, commitment, plan, car)
    }
    fn private_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("private seed root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("private seed-root mode");
        root
    }

    #[test]
    fn exact_seed_survives_restart_and_conflicting_operation_cannot_replace_it() {
        let (binding, commitment, plan, car) = fixture();
        let temporary = private_root();
        let root = temporary.path().canonicalize().expect("canonical root");
        let mut backend = MusubiSeedStagingBackendV1::open(
            &root,
            binding.seed_provider,
            2,
            MAX_STAGE_RECORD_BYTES * 2,
        )
        .expect("stage owner");
        assert_eq!(backend.provider_id(), binding.seed_provider);
        backend
            .stage_exact_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("first verified stage");
        backend
            .stage_exact_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("idempotent stage");
        backend
            .verify_staged_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("completed receipt can use exact retained bytes");
        assert_eq!(
            backend.verify_staged_car([0x62; 32], &binding, &commitment, &plan, &car),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        assert_eq!(
            backend.stage_exact_car([0x62; 32], &binding, &commitment, &plan, &car),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        let name = record_name(&binding).expect("binding key");
        let staged = backend.read_record(&name).expect("read exact stage");
        assert_eq!(staged.metadata.operation_id, [0x61; 32]);
        assert_eq!(staged.plan, plan);
        assert_eq!(staged.car, car);
        assert_eq!(backend.inventory().expect("inventory").0, 1);
        drop(backend);
        let backend = MusubiSeedStagingBackendV1::open(
            &root,
            binding.seed_provider,
            2,
            MAX_STAGE_RECORD_BYTES * 2,
        )
        .expect("restart stage owner");
        assert_eq!(backend.read_record(&name).expect("restart bytes").car, car);
        backend
            .verify_staged_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("restarted owner retains exact completed receipt bytes");
    }

    #[test]
    fn seed_root_refuses_parallel_owner_tombstones_and_capacity_overrun() {
        let (binding, commitment, plan, car) = fixture();
        let temporary = private_root();
        let root = temporary.path().canonicalize().expect("canonical root");
        let owner = MusubiSeedStagingBackendV1::open(&root, binding.seed_provider, 1, 1)
            .expect("stage owner");
        assert!(matches!(
            MusubiSeedStagingBackendV1::open(&root, binding.seed_provider, 1, 1),
            Err(MusubiSeedStagingErrorV1::Locked),
        ));
        drop(owner);
        let mut owner = MusubiSeedStagingBackendV1::open(&root, binding.seed_provider, 1, 1)
            .expect("reopened stage owner");
        assert_eq!(
            owner.stage_exact_car([0x61; 32], &binding, &commitment, &plan, &car),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable),
        );
        assert_eq!(owner.inventory().expect("unchanged inventory"), (0, 0));
        drop(owner);
        fs::write(root.join("interrupted.pending"), b"tombstone").expect("tombstone");
        assert!(matches!(
            MusubiSeedStagingBackendV1::open(&root, binding.seed_provider, 1, 1),
            Err(MusubiSeedStagingErrorV1::Invalid),
        ));
    }

    #[test]
    fn substituted_seed_file_is_never_followed_or_recovered() {
        let (binding, commitment, plan, car) = fixture();
        let temporary = private_root();
        let root = temporary.path().canonicalize().expect("canonical root");
        let mut backend = MusubiSeedStagingBackendV1::open(
            &root,
            binding.seed_provider,
            1,
            MAX_STAGE_RECORD_BYTES,
        )
        .expect("stage owner");
        backend
            .stage_exact_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("first verified stage");
        let name = record_name(&binding).expect("binding key");
        let staged = root.join(&name);
        fs::remove_file(&staged).expect("remove staged fixture");
        assert_eq!(
            backend.verify_staged_car([0x61; 32], &binding, &commitment, &plan, &car),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        assert!(
            !staged.exists(),
            "cached receipt check never recreates lost custody"
        );
        std::os::unix::fs::symlink("untrusted-target", &staged).expect("substitute symlink");
        assert!(matches!(
            backend.read_record(&name),
            Err(MusubiSeedStagingErrorV1::Invalid),
        ));
        assert_eq!(
            backend.verify_staged_car([0x61; 32], &binding, &commitment, &plan, &car),
            Err(MusubiPublicationServiceBackendErrorV1::Permanent),
        );
        drop(backend);
        assert!(matches!(
            MusubiSeedStagingBackendV1::open(
                &root,
                binding.seed_provider,
                1,
                MAX_STAGE_RECORD_BYTES
            ),
            Err(MusubiSeedStagingErrorV1::Invalid),
        ));
    }

    #[test]
    fn torn_record_is_invalid_while_transient_io_is_retryable() {
        let (binding, commitment, plan, car) = fixture();
        let temporary = private_root();
        let root = temporary.path().canonicalize().expect("canonical root");
        let mut backend = MusubiSeedStagingBackendV1::open(
            &root,
            binding.seed_provider,
            1,
            MAX_STAGE_RECORD_BYTES,
        )
        .expect("stage owner");
        backend
            .stage_exact_car([0x61; 32], &binding, &commitment, &plan, &car)
            .expect("verified stage");
        let name = record_name(&binding).expect("binding key");
        let staged = root.join(&name);
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o600))
            .expect("damage fixture permissions");
        fs::write(&staged, b"torn").expect("damage fixture record");
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o400)).expect("restore file mode");
        assert!(matches!(
            backend.read_record(&name),
            Err(MusubiSeedStagingErrorV1::Invalid),
        ));
        assert_eq!(
            read_error(std::io::Error::other("temporary device fault")),
            MusubiSeedStagingErrorV1::Unavailable,
        );
    }
}
