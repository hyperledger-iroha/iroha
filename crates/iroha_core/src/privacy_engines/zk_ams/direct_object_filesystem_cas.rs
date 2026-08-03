//! Durable append-only filesystem storage for direct ZK-AMS MKHE objects.
//!
//! The adapter owns one explicit canonical root and never turns caller bytes
//! into paths. Variable path components are lowercase hexadecimal encodings of
//! backend-generated 256-bit identities or complete typed object pointers.
//! Staging is append-only, sealing first synchronizes bytes and then moves them
//! into an immutable seal directory, and publication uses a no-overwrite hard
//! link followed by an authoritative full-content lookup. There is deliberately
//! no abort, delete, overwrite, or unpublish API.

use std::{
    collections::BTreeSet,
    ffi::OsStr,
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::{ffi::OsStrExt as _, fs::MetadataExt as _},
    path::{Component, Path, PathBuf},
};

#[cfg(test)]
use std::{fs, io::ErrorKind};

use iroha_zkp_halo2::vega::{
    ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1, ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1,
    ZkAmsMkheDirectObjectCasPublicationV1, ZkAmsMkheDirectObjectKindV1,
    ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheDirectObjectPublishedBindingV1,
    ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectSealTokenV1,
    ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1,
};
use sha3::{Digest as _, Keccak256};

const FILESYSTEM_CAS_VERSION_V1: u8 = 1;

const LAYOUT_DIRECTORY_V1: &str = "zk-ams-mkhe-direct-cas-v1";
const NAMESPACE_FILE_V1: &str = "namespace";
const INITIALIZATION_DIRECTORY_V1: &str = "initialization";
const STAGING_DIRECTORY_V1: &str = "staging";
const SEALED_DIRECTORY_V1: &str = "sealed";
const OBJECTS_DIRECTORY_V1: &str = "objects";
const RECORD_FILE_V1: &str = "record";
const PAYLOAD_FILE_V1: &str = "payload";
const POISON_FILE_V1: &str = "poison";

const NAMESPACE_RECORD_TAG_V1: [u8; 8] = *b"ZAMSN001";
const STAGING_RECORD_TAG_V1: [u8; 8] = *b"ZAMST001";
const SEAL_RECORD_TAG_V1: [u8; 8] = *b"ZAMSS001";
const NAMESPACE_RECORD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-fs.namespace-record";
const STAGING_RECORD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-fs.staging-record";
const SEAL_RECORD_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.mkhe.direct-object-fs.seal-record";
const SNAPSHOT_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-fs.snapshot-identity";
const PUBLISHED_OBJECT_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-object-fs.published-object-identity";
const POISON_MARKER_V1: &[u8] = b"ZK-AMS-MKHE-DIRECT-CAS-POISON-V1";

const NAMESPACE_RECORD_BYTES_V1: usize = 8 + 1 + 32 + 32;
const STAGING_RECORD_BYTES_V1: usize = 8 + 1 + 1 + 8 + 32 * 5;
const SEAL_RECORD_BYTES_V1: usize = 8 + 1 + 1 + 8 + 32 * 7;
const GENERATED_IDENTITY_ATTEMPTS_V1: usize = 128;

/// Native append-only filesystem CAS for direct ZK-AMS MKHE objects.
///
/// Each value is tied to one open provider/publication session. Reopening the
/// same root preserves the namespace-derived snapshot while rotating both
/// session identities, which makes pre-restart staging and seal tokens stale.
pub(super) struct ZkAmsMkheDirectObjectFilesystemCasV1 {
    root_directory: RootedDirectoryV1,
    layout_directory: RootedDirectoryV1,
    initialization_directory: RootedDirectoryV1,
    staging_directory: RootedDirectoryV1,
    sealed_directory: RootedDirectoryV1,
    objects_directory: RootedDirectoryV1,
    #[cfg(test)]
    root: PathBuf,
    #[cfg(test)]
    layout: PathBuf,
    #[cfg(test)]
    staging: PathBuf,
    #[cfg(test)]
    sealed: PathBuf,
    #[cfg(test)]
    objects: PathBuf,
    namespace_identity: [u8; 32],
    provider_identity: [u8; 32],
    snapshot_identity: [u8; 32],
    publication_identity: [u8; 32],
    poisoned_staging_identities: BTreeSet<[u8; 32]>,
    #[cfg(test)]
    lose_next_publish_ack: bool,
}

impl core::fmt::Debug for ZkAmsMkheDirectObjectFilesystemCasV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheDirectObjectFilesystemCasV1")
            .field("root", &self.root_directory.display_path)
            .field("provider_identity", &hex::encode(self.provider_identity))
            .field("snapshot_identity", &hex::encode(self.snapshot_identity))
            .field(
                "publication_identity",
                &hex::encode(self.publication_identity),
            )
            .finish_non_exhaustive()
    }
}

impl ZkAmsMkheDirectObjectFilesystemCasV1 {
    /// Open or initialize a store below one explicit absolute directory.
    ///
    /// The root itself must already exist as an owner-private real directory.
    /// The adapter opens every component without following symbolic links,
    /// retains the resulting directory descriptor, and resolves every later
    /// operation relative to retained descriptors only.
    pub(super) fn open(root: impl AsRef<Path>) -> Result<Self, ZkAmsMkheErrorV1> {
        let requested_root = root.as_ref();
        if !requested_root.is_absolute()
            || requested_root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let root_directory = RootedDirectoryV1::open_absolute(requested_root)?;
        root_directory.validate_private()?;
        let layout_directory = root_directory.open_or_create_private_child(LAYOUT_DIRECTORY_V1)?;
        let initialization_directory =
            layout_directory.open_or_create_private_child(INITIALIZATION_DIRECTORY_V1)?;
        let staging_directory =
            layout_directory.open_or_create_private_child(STAGING_DIRECTORY_V1)?;
        let sealed_directory =
            layout_directory.open_or_create_private_child(SEALED_DIRECTORY_V1)?;
        let objects_directory =
            layout_directory.open_or_create_private_child(OBJECTS_DIRECTORY_V1)?;

        let namespace_identity =
            open_or_create_namespace_identity(&layout_directory, &initialization_directory)?;
        validate_layout(
            &layout_directory,
            &initialization_directory,
            &staging_directory,
            &sealed_directory,
            &objects_directory,
        )?;
        let snapshot_identity =
            derive_nonzero_identity(SNAPSHOT_IDENTITY_DOMAIN_V1, &[&namespace_identity])?;
        let provider_identity =
            generated_distinct_identity(&[namespace_identity, snapshot_identity])?;
        let publication_identity = generated_distinct_identity(&[
            namespace_identity,
            snapshot_identity,
            provider_identity,
        ])?;

        Ok(Self {
            #[cfg(test)]
            root: requested_root.to_path_buf(),
            #[cfg(test)]
            layout: requested_root.join(LAYOUT_DIRECTORY_V1),
            #[cfg(test)]
            staging: requested_root
                .join(LAYOUT_DIRECTORY_V1)
                .join(STAGING_DIRECTORY_V1),
            #[cfg(test)]
            sealed: requested_root
                .join(LAYOUT_DIRECTORY_V1)
                .join(SEALED_DIRECTORY_V1),
            #[cfg(test)]
            objects: requested_root
                .join(LAYOUT_DIRECTORY_V1)
                .join(OBJECTS_DIRECTORY_V1),
            root_directory,
            layout_directory,
            initialization_directory,
            staging_directory,
            sealed_directory,
            objects_directory,
            namespace_identity,
            provider_identity,
            snapshot_identity,
            publication_identity,
            poisoned_staging_identities: BTreeSet::new(),
            #[cfg(test)]
            lose_next_publish_ack: false,
        })
    }

    #[cfg(test)]
    fn staging_directory(&self, identity: [u8; 32]) -> PathBuf {
        self.staging.join(hex::encode(identity))
    }

    #[cfg(test)]
    fn seal_directory(&self, identity: [u8; 32]) -> PathBuf {
        self.sealed.join(hex::encode(identity))
    }

    #[cfg(test)]
    fn object_path(&self, pointer: ZkAmsMkheDirectObjectPointerV1) -> PathBuf {
        self.objects.join(hex::encode(pointer.encode()))
    }

    fn validate_staging_token(
        &self,
        token: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<(RootedDirectoryV1, StageRecordV1), ZkAmsMkheErrorV1> {
        if token.publication_identity() != self.publication_identity
            || self
                .poisoned_staging_identities
                .contains(&token.staging_identity())
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.staging_directory.validate_private()?;
        let directory = self
            .staging_directory
            .open_private_child(&hex::encode(token.staging_identity()))?;
        validate_leaf_directory_entries(&directory, true)?;
        ensure_stage_is_not_poisoned(&directory)?;
        let record = StageRecordV1::read(&directory)?;
        record.validate(self.namespace_identity, token)?;
        Ok((directory, record))
    }

    fn validate_seal_token(
        &self,
        token: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<(RootedDirectoryV1, SealRecordV1), ZkAmsMkheErrorV1> {
        if token.publication_identity() != self.publication_identity {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.sealed_directory.validate_private()?;
        let directory = self
            .sealed_directory
            .open_private_child(&hex::encode(token.seal_identity()))?;
        validate_leaf_directory_entries(&directory, false)?;
        let record = SealRecordV1::read(&directory)?;
        record.validate(self.namespace_identity, token)?;
        Ok((directory, record))
    }

    fn validate_published_object(
        &self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        hash_payload: bool,
    ) -> Result<File, ZkAmsMkheErrorV1> {
        validate_pointer(pointer)?;
        self.objects_directory.validate_private()?;
        let name = object_name(pointer);
        let file = open_regular_file_at(
            &self.objects_directory,
            OsStr::new(&name),
            FileAccessV1::ImmutableRead,
        )?;
        let metadata = file.metadata().map_err(map_backend_error)?;
        if metadata.len() != pointer.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if hash_payload {
            let observed = hash_exact_file(
                file.try_clone().map_err(map_backend_error)?,
                pointer.payload_bytes(),
            )?;
            if observed != pointer.payload_blake3() {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
        }
        Ok(file)
    }

    fn published_object_identity(
        &self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        derive_nonzero_identity(
            PUBLISHED_OBJECT_IDENTITY_DOMAIN_V1,
            &[&self.namespace_identity, &pointer.encode()],
        )
    }

    #[cfg(test)]
    fn lose_next_publish_ack_for_test(&mut self) {
        self.lose_next_publish_ack = true;
    }
}

impl ZkAmsMkheDirectObjectCasPublicationV1 for ZkAmsMkheDirectObjectFilesystemCasV1 {
    fn publication_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.publication_identity == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(self.publication_identity)
    }

    fn begin_staging(
        &mut self,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload_bytes: u64,
    ) -> Result<ZkAmsMkheDirectObjectStagingTokenV1, ZkAmsMkheErrorV1> {
        self.staging_directory.validate_private()?;
        for _ in 0..GENERATED_IDENTITY_ATTEMPTS_V1 {
            let staging_identity = generated_distinct_identity(&[
                self.namespace_identity,
                self.provider_identity,
                self.snapshot_identity,
                self.publication_identity,
            ])?;
            let token = ZkAmsMkheDirectObjectStagingTokenV1::new(
                self.publication_identity,
                staging_identity,
                kind,
                payload_bytes,
            )?;
            let name = hex::encode(staging_identity);
            match self.staging_directory.create_private_child_new(&name) {
                Ok(directory) => {
                    let record = StageRecordV1::new(self.namespace_identity, &token)?;
                    record.write(&directory)?;
                    create_empty_mutable_file_at(&directory, OsStr::new(PAYLOAD_FILE_V1))?;
                    directory.sync()?;
                    self.staging_directory.sync()?;
                    return Ok(token);
                }
                Err(CreatePrivateChildErrorV1::AlreadyExists) => continue,
                Err(CreatePrivateChildErrorV1::Backend) => {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            }
        }
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    }

    fn staged_len(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let (directory, _) = self.validate_staging_token(staging)?;
        let file = open_regular_file_at(
            &directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::MutableAppend,
        )?;
        let length = file.metadata().map_err(map_backend_error)?.len();
        if length > staging.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(length)
    }

    fn write_staged_at(
        &mut self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        let (directory, _) = self.validate_staging_token(staging)?;
        let result = self.write_staged_at_inner(staging, &directory, absolute_offset, source);
        if !matches!(result, Ok(written) if written == source.len()) {
            // Poison in memory before attempting the durable marker. The
            // staging token is bound to this exact publication session, so a
            // process restart already makes it stale; while this session is
            // alive, marker I/O failure or same-UID marker removal must not
            // revive a write authority after an ambiguous/failed append.
            self.poisoned_staging_identities
                .insert(staging.staging_identity());
            let _ = poison_stage(&directory);
        }
        result
    }

    fn seal_staged(
        &mut self,
        staging: ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<ZkAmsMkheDirectObjectSealTokenV1, ZkAmsMkheErrorV1> {
        let (stage_directory, _) = self.validate_staging_token(&staging)?;
        let stage_file = open_regular_file_at(
            &stage_directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::MutableAppend,
        )?;
        if stage_file.metadata().map_err(map_backend_error)?.len() != staging.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        stage_file.sync_all().map_err(map_backend_error)?;
        make_open_file_immutable_at(&stage_directory, OsStr::new(PAYLOAD_FILE_V1), &stage_file)?;

        self.sealed_directory.validate_private()?;
        let mut allocated_seal = None;
        for _ in 0..GENERATED_IDENTITY_ATTEMPTS_V1 {
            let seal_identity = generated_distinct_identity(&[
                self.namespace_identity,
                self.provider_identity,
                self.snapshot_identity,
                self.publication_identity,
                staging.staging_identity(),
            ])?;
            let name = hex::encode(seal_identity);
            match self.sealed_directory.create_private_child_new(&name) {
                Ok(directory) => {
                    allocated_seal = Some((seal_identity, directory));
                    break;
                }
                Err(CreatePrivateChildErrorV1::AlreadyExists) => continue,
                Err(CreatePrivateChildErrorV1::Backend) => {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            }
        }
        let (seal_identity, seal_directory) =
            allocated_seal.ok_or(ZkAmsMkheErrorV1::RandomUnavailable)?;
        let seal = ZkAmsMkheDirectObjectSealTokenV1::from_staging(staging, seal_identity)?;
        rustix::fs::renameat(
            &stage_directory.file,
            PAYLOAD_FILE_V1,
            &seal_directory.file,
            PAYLOAD_FILE_V1,
        )
        .map_err(map_rustix_backend_error)?;
        ensure_named_file_matches_handle(
            &seal_directory,
            OsStr::new(PAYLOAD_FILE_V1),
            &stage_file,
            FileAccessV1::ImmutableRead,
        )?;
        stage_directory.sync()?;
        seal_directory.sync()?;
        self.staging_directory.sync()?;
        self.sealed_directory.sync()?;
        let record = SealRecordV1::new(self.namespace_identity, &seal)?;
        record.write(&seal_directory)?;
        seal_directory.sync()?;
        self.sealed_directory.sync()?;
        Ok(seal)
    }

    fn sealed_len(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let (directory, _) = self.validate_seal_token(seal)?;
        let file = open_regular_file_at(
            &directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::ImmutableRead,
        )?;
        let length = file.metadata().map_err(map_backend_error)?.len();
        if length != seal.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(length)
    }

    fn read_sealed_at(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        let (directory, _) = self.validate_seal_token(seal)?;
        validate_absolute_read(absolute_offset, destination.len(), seal.payload_bytes())?;
        let mut file = open_regular_file_at(
            &directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::ImmutableRead,
        )?;
        if file.metadata().map_err(map_backend_error)?.len() != seal.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        file.seek(SeekFrom::Start(absolute_offset))
            .map_err(map_backend_error)?;
        read_once_exact(&mut file, destination)
    }

    fn publish_sealed_by_pointer(
        &mut self,
        seal: &ZkAmsMkheDirectObjectSealTokenV1,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        validate_pointer(pointer)?;
        if seal.kind() != pointer.kind() || seal.payload_bytes() != pointer.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let (directory, _) = self.validate_seal_token(seal)?;
        let sealed_file = open_regular_file_at(
            &directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::ImmutableRead,
        )?;
        let observed = hash_exact_file(
            sealed_file.try_clone().map_err(map_backend_error)?,
            seal.payload_bytes(),
        )?;
        if observed != pointer.payload_blake3() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }

        self.objects_directory.validate_private()?;
        let target = object_name(pointer);
        match rustix::fs::linkat(
            &directory.file,
            PAYLOAD_FILE_V1,
            &self.objects_directory.file,
            &target,
            rustix::fs::AtFlags::empty(),
        ) {
            Ok(()) => {
                ensure_named_file_matches_handle(
                    &directory,
                    OsStr::new(PAYLOAD_FILE_V1),
                    &sealed_file,
                    FileAccessV1::ImmutableRead,
                )?;
                let target_file = open_regular_file_at(
                    &self.objects_directory,
                    OsStr::new(&target),
                    FileAccessV1::ImmutableRead,
                )?;
                if FileIdentityV1::from_metadata(
                    &target_file.metadata().map_err(map_backend_error)?,
                ) != FileIdentityV1::from_metadata(
                    &sealed_file.metadata().map_err(map_backend_error)?,
                ) {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                let published = self.validate_published_object(pointer, true)?;
                published.sync_all().map_err(map_backend_error)?;
                self.objects_directory.sync()?;
                #[cfg(test)]
                if core::mem::take(&mut self.lose_next_publish_ack) {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
                Ok(())
            }
            Err(error) if error == rustix::io::Errno::EXIST => {
                self.validate_published_object(pointer, true)?;
                Ok(())
            }
            Err(error) => Err(map_rustix_backend_error(error)),
        }
    }

    fn lookup_published_pointer(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<Option<ZkAmsMkheDirectObjectPublishedBindingV1>, ZkAmsMkheErrorV1> {
        validate_pointer(pointer)?;
        let name = object_name(pointer);
        match statat_optional(&self.objects_directory, OsStr::new(&name))? {
            Some(_) => {
                self.validate_published_object(pointer, true)?;
                Ok(Some(ZkAmsMkheDirectObjectPublishedBindingV1::new(
                    self.publication_identity,
                    self.published_object_identity(pointer)?,
                    pointer,
                )?))
            }
            None => Ok(None),
        }
    }
}

impl ZkAmsMkheDirectObjectFilesystemCasV1 {
    fn write_staged_at_inner(
        &self,
        staging: &ZkAmsMkheDirectObjectStagingTokenV1,
        directory: &RootedDirectoryV1,
        absolute_offset: u64,
        source: &[u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        if source.is_empty() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        if source.len() > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 {
            return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
        }
        let mut file = open_regular_file_at(
            directory,
            OsStr::new(PAYLOAD_FILE_V1),
            FileAccessV1::MutableAppend,
        )?;
        let before = file.metadata().map_err(map_backend_error)?.len();
        if before != absolute_offset {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let requested =
            u64::try_from(source.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let expected_after = before
            .checked_add(requested)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if expected_after > staging.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        let written = file.write(source).map_err(map_backend_error)?;
        let written_u64 =
            u64::try_from(written).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let observed_after = file.metadata().map_err(map_backend_error)?.len();
        if written != source.len()
            || observed_after
                != before
                    .checked_add(written_u64)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(written)
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for ZkAmsMkheDirectObjectFilesystemCasV1 {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.provider_identity == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(self.provider_identity)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if self.snapshot_identity == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(self.snapshot_identity)
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        let file = self.validate_published_object(pointer, false)?;
        Ok(file.metadata().map_err(map_backend_error)?.len())
    }

    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        validate_absolute_read(absolute_offset, destination.len(), pointer.payload_bytes())?;
        let mut file = self.validate_published_object(pointer, false)?;
        file.seek(SeekFrom::Start(absolute_offset))
            .map_err(map_backend_error)?;
        read_once_exact(&mut file, destination)
    }
}

#[derive(Clone, Copy)]
struct StageRecordV1 {
    namespace_identity: [u8; 32],
    publication_identity: [u8; 32],
    staging_identity: [u8; 32],
    staging_token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
}

impl StageRecordV1 {
    fn new(
        namespace_identity: [u8; 32],
        token: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            namespace_identity,
            publication_identity: token.publication_identity(),
            staging_identity: token.staging_identity(),
            staging_token_digest: token.token_digest(),
            kind: token.kind(),
            payload_bytes: token.payload_bytes(),
        };
        value.validate(namespace_identity, token)?;
        Ok(value)
    }

    fn validate(
        self,
        namespace_identity: [u8; 32],
        token: &ZkAmsMkheDirectObjectStagingTokenV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let reconstructed = ZkAmsMkheDirectObjectStagingTokenV1::new(
            self.publication_identity,
            self.staging_identity,
            self.kind,
            self.payload_bytes,
        )?;
        if namespace_identity == [0; 32]
            || self.namespace_identity != namespace_identity
            || reconstructed.publication_identity() != token.publication_identity()
            || reconstructed.staging_identity() != token.staging_identity()
            || reconstructed.kind() != token.kind()
            || reconstructed.payload_bytes() != token.payload_bytes()
            || reconstructed.token_digest() != token.token_digest()
            || self.staging_token_digest != token.token_digest()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn encode(self) -> [u8; STAGING_RECORD_BYTES_V1] {
        let mut bytes = [0_u8; STAGING_RECORD_BYTES_V1];
        bytes[..8].copy_from_slice(&STAGING_RECORD_TAG_V1);
        bytes[8] = FILESYSTEM_CAS_VERSION_V1;
        bytes[9] = self.kind as u8;
        bytes[10..18].copy_from_slice(&self.payload_bytes.to_be_bytes());
        bytes[18..50].copy_from_slice(&self.namespace_identity);
        bytes[50..82].copy_from_slice(&self.publication_identity);
        bytes[82..114].copy_from_slice(&self.staging_identity);
        bytes[114..146].copy_from_slice(&self.staging_token_digest);
        let checksum = record_checksum(STAGING_RECORD_DOMAIN_V1, &bytes[..146]);
        bytes[146..].copy_from_slice(&checksum);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != STAGING_RECORD_BYTES_V1
            || bytes[..8] != STAGING_RECORD_TAG_V1
            || bytes[8] != FILESYSTEM_CAS_VERSION_V1
            || bytes[146..] != record_checksum(STAGING_RECORD_DOMAIN_V1, &bytes[..146])
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            kind: ZkAmsMkheDirectObjectKindV1::try_from(bytes[9])?,
            payload_bytes: read_u64(&bytes[10..18]),
            namespace_identity: read_32(&bytes[18..50]),
            publication_identity: read_32(&bytes[50..82]),
            staging_identity: read_32(&bytes[82..114]),
            staging_token_digest: read_32(&bytes[114..146]),
        })
    }

    fn write(self, directory: &RootedDirectoryV1) -> Result<(), ZkAmsMkheErrorV1> {
        create_immutable_file_at(directory, OsStr::new(RECORD_FILE_V1), &self.encode())
    }

    fn read(directory: &RootedDirectoryV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let bytes = read_fixed_immutable_file_at::<STAGING_RECORD_BYTES_V1>(
            directory,
            OsStr::new(RECORD_FILE_V1),
        )?;
        Self::decode(&bytes)
    }
}

#[derive(Clone, Copy)]
struct SealRecordV1 {
    namespace_identity: [u8; 32],
    publication_identity: [u8; 32],
    staging_identity: [u8; 32],
    staging_token_digest: [u8; 32],
    seal_identity: [u8; 32],
    seal_token_digest: [u8; 32],
    kind: ZkAmsMkheDirectObjectKindV1,
    payload_bytes: u64,
}

impl SealRecordV1 {
    fn new(
        namespace_identity: [u8; 32],
        token: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let value = Self {
            namespace_identity,
            publication_identity: token.publication_identity(),
            staging_identity: token.staging_identity(),
            staging_token_digest: token.staging_token_digest(),
            seal_identity: token.seal_identity(),
            seal_token_digest: token.token_digest(),
            kind: token.kind(),
            payload_bytes: token.payload_bytes(),
        };
        value.validate(namespace_identity, token)?;
        Ok(value)
    }

    fn validate(
        self,
        namespace_identity: [u8; 32],
        token: &ZkAmsMkheDirectObjectSealTokenV1,
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let stage = ZkAmsMkheDirectObjectStagingTokenV1::new(
            self.publication_identity,
            self.staging_identity,
            self.kind,
            self.payload_bytes,
        )?;
        if stage.token_digest() != self.staging_token_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let reconstructed =
            ZkAmsMkheDirectObjectSealTokenV1::from_staging(stage, self.seal_identity)?;
        if namespace_identity == [0; 32]
            || self.namespace_identity != namespace_identity
            || reconstructed.publication_identity() != token.publication_identity()
            || reconstructed.staging_identity() != token.staging_identity()
            || reconstructed.staging_token_digest() != token.staging_token_digest()
            || reconstructed.seal_identity() != token.seal_identity()
            || reconstructed.kind() != token.kind()
            || reconstructed.payload_bytes() != token.payload_bytes()
            || reconstructed.token_digest() != token.token_digest()
            || self.seal_token_digest != token.token_digest()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn encode(self) -> [u8; SEAL_RECORD_BYTES_V1] {
        let mut bytes = [0_u8; SEAL_RECORD_BYTES_V1];
        bytes[..8].copy_from_slice(&SEAL_RECORD_TAG_V1);
        bytes[8] = FILESYSTEM_CAS_VERSION_V1;
        bytes[9] = self.kind as u8;
        bytes[10..18].copy_from_slice(&self.payload_bytes.to_be_bytes());
        bytes[18..50].copy_from_slice(&self.namespace_identity);
        bytes[50..82].copy_from_slice(&self.publication_identity);
        bytes[82..114].copy_from_slice(&self.staging_identity);
        bytes[114..146].copy_from_slice(&self.staging_token_digest);
        bytes[146..178].copy_from_slice(&self.seal_identity);
        bytes[178..210].copy_from_slice(&self.seal_token_digest);
        let checksum = record_checksum(SEAL_RECORD_DOMAIN_V1, &bytes[..210]);
        bytes[210..].copy_from_slice(&checksum);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, ZkAmsMkheErrorV1> {
        if bytes.len() != SEAL_RECORD_BYTES_V1
            || bytes[..8] != SEAL_RECORD_TAG_V1
            || bytes[8] != FILESYSTEM_CAS_VERSION_V1
            || bytes[210..] != record_checksum(SEAL_RECORD_DOMAIN_V1, &bytes[..210])
        {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        Ok(Self {
            kind: ZkAmsMkheDirectObjectKindV1::try_from(bytes[9])?,
            payload_bytes: read_u64(&bytes[10..18]),
            namespace_identity: read_32(&bytes[18..50]),
            publication_identity: read_32(&bytes[50..82]),
            staging_identity: read_32(&bytes[82..114]),
            staging_token_digest: read_32(&bytes[114..146]),
            seal_identity: read_32(&bytes[146..178]),
            seal_token_digest: read_32(&bytes[178..210]),
        })
    }

    fn write(self, directory: &RootedDirectoryV1) -> Result<(), ZkAmsMkheErrorV1> {
        create_immutable_file_at(directory, OsStr::new(RECORD_FILE_V1), &self.encode())
    }

    fn read(directory: &RootedDirectoryV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let bytes = read_fixed_immutable_file_at::<SEAL_RECORD_BYTES_V1>(
            directory,
            OsStr::new(RECORD_FILE_V1),
        )?;
        Self::decode(&bytes)
    }
}

fn open_or_create_namespace_identity(
    layout: &RootedDirectoryV1,
    initialization: &RootedDirectoryV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if statat_optional(layout, OsStr::new(NAMESPACE_FILE_V1))?.is_some() {
        return read_namespace_identity(layout, OsStr::new(NAMESPACE_FILE_V1));
    }

    let identity = generated_distinct_identity(&[])?;
    let candidate_name = hex::encode(identity);
    create_immutable_file_at(
        initialization,
        OsStr::new(&candidate_name),
        &namespace_record(identity),
    )?;
    initialization.sync()?;
    match rustix::fs::linkat(
        &initialization.file,
        &candidate_name,
        &layout.file,
        NAMESPACE_FILE_V1,
        rustix::fs::AtFlags::empty(),
    ) {
        Ok(()) => {
            layout.sync()?;
            let installed = read_namespace_identity(layout, OsStr::new(NAMESPACE_FILE_V1))?;
            if installed != identity {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            Ok(installed)
        }
        Err(error) if error == rustix::io::Errno::EXIST => {
            read_namespace_identity(layout, OsStr::new(NAMESPACE_FILE_V1))
        }
        Err(error) => Err(map_rustix_backend_error(error)),
    }
}

fn namespace_record(identity: [u8; 32]) -> [u8; NAMESPACE_RECORD_BYTES_V1] {
    let mut bytes = [0_u8; NAMESPACE_RECORD_BYTES_V1];
    bytes[..8].copy_from_slice(&NAMESPACE_RECORD_TAG_V1);
    bytes[8] = FILESYSTEM_CAS_VERSION_V1;
    bytes[9..41].copy_from_slice(&identity);
    let checksum = record_checksum(NAMESPACE_RECORD_DOMAIN_V1, &bytes[..41]);
    bytes[41..].copy_from_slice(&checksum);
    bytes
}

fn read_namespace_identity(
    directory: &RootedDirectoryV1,
    name: &OsStr,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let bytes = read_fixed_immutable_file_at::<NAMESPACE_RECORD_BYTES_V1>(directory, name)?;
    if bytes[..8] != NAMESPACE_RECORD_TAG_V1
        || bytes[8] != FILESYSTEM_CAS_VERSION_V1
        || bytes[41..] != record_checksum(NAMESPACE_RECORD_DOMAIN_V1, &bytes[..41])
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let identity = read_32(&bytes[9..41]);
    if identity == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(identity)
}

fn validate_layout(
    layout: &RootedDirectoryV1,
    initialization: &RootedDirectoryV1,
    staging: &RootedDirectoryV1,
    sealed: &RootedDirectoryV1,
    objects: &RootedDirectoryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    layout.validate_private()?;
    let observed = layout.entry_names()?;
    for name in &observed {
        if !matches!(
            name.as_str(),
            NAMESPACE_FILE_V1
                | INITIALIZATION_DIRECTORY_V1
                | STAGING_DIRECTORY_V1
                | SEALED_DIRECTORY_V1
                | OBJECTS_DIRECTORY_V1
        ) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    if observed.len() != 5 {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    initialization.validate_private()?;
    staging.validate_private()?;
    sealed.validate_private()?;
    objects.validate_private()?;
    validate_initialization_directory(initialization)?;
    validate_identity_directory(staging, true)?;
    validate_identity_directory(sealed, false)?;
    validate_object_directory(objects)?;
    Ok(())
}

fn validate_initialization_directory(
    initialization: &RootedDirectoryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    for name in initialization.entry_names()? {
        require_canonical_hex(&name, 32)?;
        let stat = statat_required(initialization, OsStr::new(&name))?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        // A create-new candidate is visible while its owning opener writes and
        // synchronizes it. Only the immutable, exact-width state is eligible
        // for validation or namespace installation; a crash may leave an
        // incomplete candidate, which is an unreachable append-only orphan.
        if u64::try_from(stat.st_size).ok() != Some(NAMESPACE_RECORD_BYTES_V1 as u64)
            || !is_immutable_stat(&stat)
        {
            continue;
        }
        let identity = read_namespace_identity(initialization, OsStr::new(&name))?;
        if name != hex::encode(identity) {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}

fn validate_identity_directory(
    parent: &RootedDirectoryV1,
    staging: bool,
) -> Result<(), ZkAmsMkheErrorV1> {
    for name in parent.entry_names()? {
        require_canonical_hex(&name, 32)?;
        let directory = parent.open_private_child(&name)?;
        validate_leaf_directory_entries(&directory, staging)?;
    }
    Ok(())
}

fn validate_leaf_directory_entries(
    directory: &RootedDirectoryV1,
    staging: bool,
) -> Result<(), ZkAmsMkheErrorV1> {
    for name in directory.entry_names()? {
        let allowed = name == RECORD_FILE_V1
            || name == PAYLOAD_FILE_V1
            || (staging && name == POISON_FILE_V1);
        if !allowed {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let stat = statat_required(directory, OsStr::new(&name))?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
    }
    Ok(())
}

fn validate_object_directory(objects: &RootedDirectoryV1) -> Result<(), ZkAmsMkheErrorV1> {
    for name in objects.entry_names()? {
        let pointer = pointer_from_filename(&name)?;
        let file = open_regular_file_at(objects, OsStr::new(&name), FileAccessV1::ImmutableRead)?;
        if file.metadata().map_err(map_backend_error)?.len() != pointer.payload_bytes() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
    }
    Ok(())
}

fn object_name(pointer: ZkAmsMkheDirectObjectPointerV1) -> String {
    hex::encode(pointer.encode())
}

fn pointer_from_filename(name: &str) -> Result<ZkAmsMkheDirectObjectPointerV1, ZkAmsMkheErrorV1> {
    require_canonical_hex(name, ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1)?;
    let bytes = hex::decode(name).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    if bytes.len() != ZK_AMS_MKHE_DIRECT_OBJECT_POINTER_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let kind = ZkAmsMkheDirectObjectKindV1::try_from(bytes[5])?;
    let pointer = ZkAmsMkheDirectObjectPointerV1::decode_exact(kind, &bytes)?;
    validate_pointer(pointer)?;
    Ok(pointer)
}

fn validate_pointer(pointer: ZkAmsMkheDirectObjectPointerV1) -> Result<(), ZkAmsMkheErrorV1> {
    let decoded = ZkAmsMkheDirectObjectPointerV1::decode_exact(pointer.kind(), &pointer.encode())?;
    if decoded != pointer {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn validate_absolute_read(
    absolute_offset: u64,
    requested: usize,
    payload_bytes: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    if requested == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    if requested > ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    let requested =
        u64::try_from(requested).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let end = absolute_offset
        .checked_add(requested)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if end > payload_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn hash_exact_file(mut file: File, payload_bytes: u64) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    if file.metadata().map_err(map_backend_error)?.len() != payload_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    file.seek(SeekFrom::Start(0)).map_err(map_backend_error)?;
    let mut hasher = norito::streaming::Blake3Hasher::new();
    let mut remaining = payload_bytes;
    let mut buffer = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
    while remaining != 0 {
        let take = buffer.len().min(
            usize::try_from(remaining).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
        );
        file.read_exact(&mut buffer[..take])
            .map_err(map_backend_error)?;
        hasher.update(&buffer[..take]);
        remaining = remaining
            .checked_sub(
                u64::try_from(take).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
    }
    if file.metadata().map_err(map_backend_error)?.len() != payload_bytes {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(hasher.finalize())
}

fn read_once_exact(file: &mut File, destination: &mut [u8]) -> Result<usize, ZkAmsMkheErrorV1> {
    let read = file.read(destination).map_err(map_backend_error)?;
    if read != destination.len() {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(read)
}

fn record_checksum(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(bytes);
    hash.finalize().into()
}

fn derive_nonzero_identity(
    domain: &[u8],
    components: &[&[u8]],
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(domain);
    hash.update(&[FILESYSTEM_CAS_VERSION_V1]);
    for component in components {
        let length = u64::try_from(component.len())
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        hash.update(&length.to_be_bytes());
        hash.update(component);
    }
    let identity: [u8; 32] = hash.finalize().into();
    if identity == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(identity)
}

fn generated_distinct_identity(forbidden: &[[u8; 32]]) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    for _ in 0..GENERATED_IDENTITY_ATTEMPTS_V1 {
        let mut identity = [0_u8; 32];
        fill_os_random(&mut identity)?;
        if identity != [0; 32] && !forbidden.contains(&identity) {
            return Ok(identity);
        }
    }
    Err(ZkAmsMkheErrorV1::RandomUnavailable)
}

fn fill_os_random(destination: &mut [u8]) -> Result<(), ZkAmsMkheErrorV1> {
    rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, destination)
        .map_err(|_| ZkAmsMkheErrorV1::RandomUnavailable)
}

fn read_u64(bytes: &[u8]) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(bytes);
    u64::from_be_bytes(value)
}

fn read_32(bytes: &[u8]) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(bytes);
    value
}

fn require_canonical_hex(value: &str, decoded_bytes: usize) -> Result<(), ZkAmsMkheErrorV1> {
    if value.len() != decoded_bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    Ok(())
}

fn map_backend_error(_error: std::io::Error) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidKeyMaterial
}

fn map_rustix_backend_error(_error: rustix::io::Errno) -> ZkAmsMkheErrorV1 {
    ZkAmsMkheErrorV1::InvalidKeyMaterial
}

fn create_empty_mutable_file_at(
    directory: &RootedDirectoryV1,
    name: &OsStr,
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_component_name(name)?;
    let file = File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .map_err(map_rustix_backend_error)?,
    );
    file.sync_all().map_err(map_backend_error)?;
    ensure_named_file_matches_handle(directory, name, &file, FileAccessV1::MutableAppend)?;
    directory.sync()
}

fn create_immutable_file_at(
    directory: &RootedDirectoryV1,
    name: &OsStr,
    bytes: &[u8],
) -> Result<(), ZkAmsMkheErrorV1> {
    validate_component_name(name)?;
    let mut file = File::from(
        rustix::fs::openat(
            &directory.file,
            name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .map_err(map_rustix_backend_error)?,
    );
    file.write_all(bytes).map_err(map_backend_error)?;
    file.sync_all().map_err(map_backend_error)?;
    make_open_file_immutable_at(directory, name, &file)?;
    directory.sync()
}

fn make_open_file_immutable_at(
    directory: &RootedDirectoryV1,
    name: &OsStr,
    file: &File,
) -> Result<(), ZkAmsMkheErrorV1> {
    rustix::fs::fchmod(file, rustix::fs::Mode::from_raw_mode(0o400))
        .map_err(map_rustix_backend_error)?;
    file.sync_all().map_err(map_backend_error)?;
    ensure_named_file_matches_handle(directory, name, file, FileAccessV1::ImmutableRead)
}

fn read_fixed_immutable_file_at<const N: usize>(
    directory: &RootedDirectoryV1,
    name: &OsStr,
) -> Result<[u8; N], ZkAmsMkheErrorV1> {
    let mut file = open_regular_file_at(directory, name, FileAccessV1::ImmutableRead)?;
    if file.metadata().map_err(map_backend_error)?.len()
        != u64::try_from(N).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
    }
    let mut bytes = [0_u8; N];
    file.read_exact(&mut bytes).map_err(map_backend_error)?;
    Ok(bytes)
}

fn poison_stage(directory: &RootedDirectoryV1) -> Result<(), ZkAmsMkheErrorV1> {
    match create_immutable_file_at(directory, OsStr::new(POISON_FILE_V1), POISON_MARKER_V1) {
        Ok(()) => {
            directory.sync()?;
            Ok(())
        }
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
            if statat_optional(directory, OsStr::new(POISON_FILE_V1))?.is_some() =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn ensure_stage_is_not_poisoned(directory: &RootedDirectoryV1) -> Result<(), ZkAmsMkheErrorV1> {
    match statat_optional(directory, OsStr::new(POISON_FILE_V1))? {
        None => Ok(()),
        Some(_) => Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
    }
}

#[derive(Clone, Copy)]
enum FileAccessV1 {
    MutableAppend,
    ImmutableRead,
}

fn open_regular_file_at(
    directory: &RootedDirectoryV1,
    name: &OsStr,
    access: FileAccessV1,
) -> Result<File, ZkAmsMkheErrorV1> {
    validate_component_name(name)?;
    directory.validate_private()?;
    let before = statat_required(directory, name)?;
    validate_file_stat(&before, access)?;
    let flags = rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC
        | match access {
            FileAccessV1::MutableAppend => rustix::fs::OFlags::RDWR | rustix::fs::OFlags::APPEND,
            FileAccessV1::ImmutableRead => rustix::fs::OFlags::RDONLY,
        };
    let file = File::from(
        rustix::fs::openat(&directory.file, name, flags, rustix::fs::Mode::empty())
            .map_err(map_rustix_backend_error)?,
    );
    let opened = file.metadata().map_err(map_backend_error)?;
    let after = statat_required(directory, name)?;
    if !metadata_matches_stat(&opened, &before)
        || !metadata_matches_stat(&opened, &after)
        || validate_file_metadata(&opened, access).is_err()
        || validate_file_stat(&after, access).is_err()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(file)
}

fn ensure_named_file_matches_handle(
    directory: &RootedDirectoryV1,
    name: &OsStr,
    file: &File,
    access: FileAccessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let metadata = file.metadata().map_err(map_backend_error)?;
    let named = statat_required(directory, name)?;
    validate_file_metadata(&metadata, access)?;
    validate_file_stat(&named, access)?;
    if !metadata_matches_stat(&metadata, &named) {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_file_metadata(
    metadata: &Metadata,
    access: FileAccessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected_mode = match access {
        FileAccessV1::MutableAppend => 0o600,
        FileAccessV1::ImmutableRead => 0o400,
    };
    if !metadata.is_file()
        || metadata.uid() != current_uid()
        || metadata.mode() & 0o777 != expected_mode
        || metadata.nlink() == 0
        || (matches!(access, FileAccessV1::MutableAppend) && metadata.nlink() != 1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn validate_file_stat(
    stat: &rustix::fs::Stat,
    access: FileAccessV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    let expected_mode = match access {
        FileAccessV1::MutableAppend => 0o600,
        FileAccessV1::ImmutableRead => 0o400,
    };
    let links = u64::try_from(stat.st_nlink).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
        || stat.st_uid != current_uid()
        || u32::from(stat.st_mode) & 0o777 != expected_mode
        || links == 0
        || (matches!(access, FileAccessV1::MutableAppend) && links != 1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn is_immutable_stat(stat: &rustix::fs::Stat) -> bool {
    rustix::fs::FileType::from_raw_mode(stat.st_mode) == rustix::fs::FileType::RegularFile
        && stat.st_uid == current_uid()
        && u32::from(stat.st_mode) & 0o777 == 0o400
}

fn metadata_matches_stat(metadata: &Metadata, stat: &rustix::fs::Stat) -> bool {
    u64::try_from(stat.st_dev).ok() == Some(metadata.dev())
        && u64::try_from(stat.st_ino).ok() == Some(metadata.ino())
        && u64::try_from(stat.st_nlink).ok() == Some(metadata.nlink())
        && u64::try_from(stat.st_size).ok() == Some(metadata.len())
}

fn statat_required(
    directory: &RootedDirectoryV1,
    name: &OsStr,
) -> Result<rustix::fs::Stat, ZkAmsMkheErrorV1> {
    validate_component_name(name)?;
    rustix::fs::statat(&directory.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(map_rustix_backend_error)
}

fn statat_optional(
    directory: &RootedDirectoryV1,
    name: &OsStr,
) -> Result<Option<rustix::fs::Stat>, ZkAmsMkheErrorV1> {
    validate_component_name(name)?;
    match rustix::fs::statat(&directory.file, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Ok(Some(stat)),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(error) => Err(map_rustix_backend_error(error)),
    }
}

fn validate_component_name(name: &OsStr) -> Result<(), ZkAmsMkheErrorV1> {
    let bytes = name.as_bytes();
    if bytes.is_empty()
        || matches!(bytes, b"." | b"..")
        || bytes.contains(&b'/')
        || bytes.contains(&0)
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(())
}

fn current_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentityV1 {
    device: u64,
    inode: u64,
}

impl FileIdentityV1 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Result<Self, ZkAmsMkheErrorV1> {
        Ok(Self {
            device: u64::try_from(stat.st_dev).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
            inode: u64::try_from(stat.st_ino).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?,
        })
    }
}

struct RootedDirectoryV1 {
    file: File,
    identity: FileIdentityV1,
    display_path: PathBuf,
}

impl RootedDirectoryV1 {
    fn open_absolute(path: &Path) -> Result<Self, ZkAmsMkheErrorV1> {
        if !path.is_absolute() {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut current = File::from(
            rustix::fs::open(
                Path::new("/"),
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(map_rustix_backend_error)?,
        );
        let mut components = 0_usize;
        for component in path.components() {
            let name = match component {
                Component::RootDir => continue,
                Component::Normal(name) => name,
                Component::CurDir | Component::ParentDir | Component::Prefix(_) => {
                    return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
                }
            };
            components = components
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            if components > 128 {
                return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
            }
            validate_component_name(name)?;
            let before = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(map_rustix_backend_error)?;
            if rustix::fs::FileType::from_raw_mode(before.st_mode)
                != rustix::fs::FileType::Directory
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            let expected = FileIdentityV1::from_stat(&before)?;
            let child = File::from(
                rustix::fs::openat(
                    &current,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(map_rustix_backend_error)?,
            );
            let opened = child.metadata().map_err(map_backend_error)?;
            let after = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                .map_err(map_rustix_backend_error)?;
            if !opened.is_dir()
                || FileIdentityV1::from_metadata(&opened) != expected
                || FileIdentityV1::from_stat(&after)? != expected
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            current = child;
        }
        let metadata = current.metadata().map_err(map_backend_error)?;
        let directory = Self {
            identity: FileIdentityV1::from_metadata(&metadata),
            file: current,
            display_path: path.to_path_buf(),
        };
        directory.validate_private()?;
        Ok(directory)
    }

    fn validate_private(&self) -> Result<(), ZkAmsMkheErrorV1> {
        let metadata = self.file.metadata().map_err(map_backend_error)?;
        if !metadata.is_dir()
            || FileIdentityV1::from_metadata(&metadata) != self.identity
            || metadata.uid() != current_uid()
            || metadata.mode() & 0o777 != 0o700
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(())
    }

    fn open_private_child(&self, name: &str) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_component_name(OsStr::new(name))?;
        self.validate_private()?;
        let before = statat_required(self, OsStr::new(name))?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let expected = FileIdentityV1::from_stat(&before)?;
        let file = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(map_rustix_backend_error)?,
        );
        let metadata = file.metadata().map_err(map_backend_error)?;
        let after = statat_required(self, OsStr::new(name))?;
        if !metadata.is_dir()
            || FileIdentityV1::from_metadata(&metadata) != expected
            || FileIdentityV1::from_stat(&after)? != expected
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let child = Self {
            file,
            identity: expected,
            display_path: self.display_path.join(name),
        };
        child.validate_private()?;
        Ok(child)
    }

    fn open_or_create_private_child(&self, name: &str) -> Result<Self, ZkAmsMkheErrorV1> {
        validate_component_name(OsStr::new(name))?;
        let created =
            match rustix::fs::mkdirat(&self.file, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) => true,
                Err(error) if error == rustix::io::Errno::EXIST => false,
                Err(error) => return Err(map_rustix_backend_error(error)),
            };
        let child = self.open_private_child(name)?;
        if created {
            self.sync()?;
        }
        Ok(child)
    }

    fn create_private_child_new(&self, name: &str) -> Result<Self, CreatePrivateChildErrorV1> {
        if validate_component_name(OsStr::new(name)).is_err() || self.validate_private().is_err() {
            return Err(CreatePrivateChildErrorV1::Backend);
        }
        match rustix::fs::mkdirat(&self.file, name, rustix::fs::Mode::from_raw_mode(0o700)) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::EXIST => {
                return Err(CreatePrivateChildErrorV1::AlreadyExists);
            }
            Err(_) => return Err(CreatePrivateChildErrorV1::Backend),
        }
        let child = self
            .open_private_child(name)
            .map_err(|_| CreatePrivateChildErrorV1::Backend)?;
        self.sync()
            .map_err(|_| CreatePrivateChildErrorV1::Backend)?;
        Ok(child)
    }

    fn entry_names(&self) -> Result<Vec<String>, ZkAmsMkheErrorV1> {
        self.validate_private()?;
        let mut names = Vec::new();
        let mut entries =
            rustix::fs::Dir::read_from(&self.file).map_err(map_rustix_backend_error)?;
        for entry in &mut entries {
            let entry = entry.map_err(map_rustix_backend_error)?;
            let bytes = entry.file_name().to_bytes();
            if matches!(bytes, b"." | b"..") {
                continue;
            }
            if names.len() >= 1_000_000 {
                return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
            }
            let name =
                core::str::from_utf8(bytes).map_err(|_| ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
            validate_component_name(OsStr::new(name))?;
            names.push(name.to_owned());
        }
        names.sort_unstable();
        Ok(names)
    }

    fn sync(&self) -> Result<(), ZkAmsMkheErrorV1> {
        self.validate_private()?;
        self.file.sync_all().map_err(map_backend_error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CreatePrivateChildErrorV1 {
    AlreadyExists,
    Backend,
}

#[cfg(test)]
fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(test)]
fn make_file_immutable(path: &Path) -> Result<(), ZkAmsMkheErrorV1> {
    use std::os::unix::fs::PermissionsExt as _;

    let metadata = fs::symlink_metadata(path).map_err(map_backend_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o400);
    fs::set_permissions(path, permissions).map_err(map_backend_error)
}

#[cfg(test)]
fn create_immutable_file(path: &Path, bytes: &[u8]) -> Result<(), ZkAmsMkheErrorV1> {
    use std::os::unix::fs::OpenOptionsExt as _;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(map_backend_error)?;
    file.write_all(bytes).map_err(map_backend_error)?;
    file.sync_all().map_err(map_backend_error)?;
    make_file_immutable(path)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Seek as _, Write as _},
        sync::{Arc, Barrier},
        thread,
    };

    use super::*;
    use iroha_zkp_halo2::vega::{
        ZkAmsMkheDirectObjectPublicationReceiptV1, ZkAmsMkheDirectObjectPublicationTransactionV1,
        validate_zk_ams_mkhe_direct_object_v1,
    };

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let base = fs::canonicalize(std::env::temp_dir()).unwrap();
            for _ in 0..GENERATED_IDENTITY_ATTEMPTS_V1 {
                let identity = generated_distinct_identity(&[]).unwrap();
                let path = base.join(format!("iroha-zkams-direct-cas-{}", hex::encode(identity)));
                match create_private_directory(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("failed to allocate test root: {error}"),
                }
            }
            panic!("failed to allocate collision-free test root")
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_payload(bytes: usize, salt: u8) -> Vec<u8> {
        (0..bytes)
            .map(|index| (index as u8).wrapping_mul(29).wrapping_add(salt))
            .collect()
    }

    fn publish(
        store: &mut ZkAmsMkheDirectObjectFilesystemCasV1,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload: &[u8],
    ) -> ZkAmsMkheDirectObjectPublicationReceiptV1 {
        let mut transaction =
            ZkAmsMkheDirectObjectPublicationTransactionV1::begin(kind, payload.len() as u64, store)
                .unwrap();
        for chunk in payload.chunks(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1) {
            transaction.write_exact(chunk).unwrap();
        }
        transaction.finish().unwrap()
    }

    fn stage_and_seal(
        store: &mut ZkAmsMkheDirectObjectFilesystemCasV1,
        kind: ZkAmsMkheDirectObjectKindV1,
        payload: &[u8],
    ) -> ZkAmsMkheDirectObjectSealTokenV1 {
        let stage = store.begin_staging(kind, payload.len() as u64).unwrap();
        let mut offset = 0_u64;
        for chunk in payload.chunks(ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1) {
            assert_eq!(
                store.write_staged_at(&stage, offset, chunk).unwrap(),
                chunk.len()
            );
            offset += chunk.len() as u64;
        }
        store.seal_staged(stage).unwrap()
    }

    fn make_test_file_mutable(path: &Path) {
        let metadata = fs::symlink_metadata(path).unwrap();
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o600);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions).unwrap();
    }

    fn overwrite_test_file(path: &Path, payload: &[u8]) {
        make_test_file_mutable(path);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap();
        file.write_all(payload).unwrap();
        file.sync_all().unwrap();
        make_file_immutable(path).unwrap();
    }

    #[test]
    fn publishes_reads_and_reopens_with_stable_snapshot_and_rotated_sessions() {
        let root = TestRoot::new();
        let payload = fixture_payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 913, 0x31);
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let provider_before = store.provider_identity().unwrap();
        let publication_before = store.publication_identity().unwrap();
        let snapshot_before = store.snapshot_identity().unwrap();
        assert_ne!(provider_before, [0; 32]);
        assert_ne!(publication_before, [0; 32]);
        assert_ne!(snapshot_before, [0; 32]);
        assert_ne!(provider_before, publication_before);
        let receipt = publish(&mut store, ZkAmsMkheDirectObjectKindV1::CpkPartyB, &payload);
        let pointer = receipt.pointer();
        let published_identity = receipt.published_binding().published_object_identity();
        assert_eq!(
            receipt.post_publish_read_receipt().canonical_bytes(),
            payload.len() as u64
        );
        assert!(!receipt.reconciled_after_publish_error());
        assert_eq!(
            store
                .object_path(pointer)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            hex::encode(pointer.encode())
        );
        drop(store);

        let mut reopened = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        assert_eq!(reopened.snapshot_identity().unwrap(), snapshot_before);
        assert_ne!(reopened.provider_identity().unwrap(), provider_before);
        assert_ne!(reopened.publication_identity().unwrap(), publication_before);
        let binding = reopened.lookup_published_pointer(pointer).unwrap().unwrap();
        assert_eq!(binding.published_object_identity(), published_identity);
        let reread = validate_zk_ams_mkhe_direct_object_v1(
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            pointer,
            &mut reopened,
        )
        .unwrap();
        assert_eq!(reread.canonical_bytes(), payload.len() as u64);
        assert_eq!(reread.payload_blake3(), pointer.payload_blake3());
    }

    #[test]
    fn concurrent_same_pointer_publication_is_atomic_and_idempotent() {
        let root = TestRoot::new();
        drop(ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap());
        let payload = Arc::new(fixture_payload(13_337, 0x44));
        let barrier = Arc::new(Barrier::new(4));
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let path = root.0.clone();
                let payload = Arc::clone(&payload);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(path).unwrap();
                    barrier.wait();
                    publish(
                        &mut store,
                        ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
                        &payload,
                    )
                    .pointer()
                })
            })
            .collect();
        let pointers: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(pointers.iter().all(|pointer| *pointer == pointers[0]));
        let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        assert_eq!(fs::read_dir(&store.objects).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_first_open_installs_one_complete_namespace_record() {
        let root = TestRoot::new();
        let barrier = Arc::new(Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let path = root.0.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(path).unwrap();
                    (store.namespace_identity, store.snapshot_identity)
                })
            })
            .collect();
        let identities: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(identities.iter().all(|identity| *identity == identities[0]));
        assert_ne!(identities[0].0, [0; 32]);
        assert_ne!(identities[0].1, [0; 32]);
    }

    #[test]
    fn lost_publish_ack_is_reconciled_by_authoritative_pointer_lookup() {
        let root = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        store.lose_next_publish_ack_for_test();
        let receipt = publish(
            &mut store,
            ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
            &fixture_payload(777, 0x55),
        );
        assert!(receipt.reconciled_after_publish_error());
        assert!(
            store
                .lookup_published_pointer(receipt.pointer())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn wrong_kind_length_and_hash_cannot_publish_or_validate() {
        let root = TestRoot::new();
        let payload = fixture_payload(257, 0x61);
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let seal = stage_and_seal(&mut store, ZkAmsMkheDirectObjectKindV1::CpkPartyB, &payload);
        let wrong_kind = ZkAmsMkheDirectObjectPointerV1::from_payload(
            ZkAmsMkheDirectObjectKindV1::ProofEnvelope,
            &payload,
        )
        .unwrap();
        assert!(store.publish_sealed_by_pointer(&seal, wrong_kind).is_err());
        let correct = ZkAmsMkheDirectObjectPointerV1::from_payload(seal.kind(), &payload).unwrap();
        store.publish_sealed_by_pointer(&seal, correct).unwrap();

        let longer = ZkAmsMkheDirectObjectPointerV1::new(
            correct.kind(),
            correct.payload_bytes() + 1,
            correct.payload_blake3(),
        )
        .unwrap();
        fs::hard_link(store.object_path(correct), store.object_path(longer)).unwrap();
        assert!(store.object_len(longer).is_err());

        let wrong_hash = ZkAmsMkheDirectObjectPointerV1::from_payload(
            correct.kind(),
            &fixture_payload(payload.len(), 0x62),
        )
        .unwrap();
        fs::hard_link(store.object_path(correct), store.object_path(wrong_hash)).unwrap();
        assert!(store.lookup_published_pointer(wrong_hash).is_err());
        assert!(
            validate_zk_ams_mkhe_direct_object_v1(correct.kind(), wrong_hash, &mut store).is_err()
        );
    }

    #[test]
    fn poisoned_write_authority_cannot_be_retried() {
        let root = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 4)
            .unwrap();
        assert!(store.write_staged_at(&stage, 1, &[1]).is_err());
        assert!(store.staged_len(&stage).is_err());
        assert!(store.write_staged_at(&stage, 0, &[1, 2, 3, 4]).is_err());

        let empty = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 1)
            .unwrap();
        assert!(store.write_staged_at(&empty, 0, &[]).is_err());
        assert!(store.staged_len(&empty).is_err());
        let oversized = store
            .begin_staging(
                ZkAmsMkheDirectObjectKindV1::CpkPartyB,
                (ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1) as u64,
            )
            .unwrap();
        assert_eq!(
            store.write_staged_at(
                &oversized,
                0,
                &vec![0xAA; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1],
            ),
            Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
        );
        assert!(store.staged_len(&oversized).is_err());
    }

    #[test]
    fn removed_poison_marker_cannot_revive_failed_stage_in_live_session() {
        let root = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 4)
            .unwrap();

        assert!(store.write_staged_at(&stage, 1, b"x").is_err());
        let poison = store
            .staging_directory(stage.staging_identity())
            .join(POISON_FILE_V1);
        assert!(poison.is_file());
        fs::remove_file(poison).unwrap();

        assert!(store.staged_len(&stage).is_err());
        assert!(store.write_staged_at(&stage, 0, b"safe").is_err());
        assert!(store.seal_staged(stage).is_err());
    }

    #[test]
    fn stale_and_forged_move_only_tokens_are_rejected() {
        let root = TestRoot::new();
        let mut first = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let stale = first
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 9)
            .unwrap();
        let old_publication = first.publication_identity().unwrap();
        drop(first);
        let mut reopened = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        assert_ne!(reopened.publication_identity().unwrap(), old_publication);
        assert!(reopened.staged_len(&stale).is_err());
        let forged_stage = ZkAmsMkheDirectObjectStagingTokenV1::new(
            reopened.publication_identity().unwrap(),
            [0xA7; 32],
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            9,
        )
        .unwrap();
        assert!(reopened.staged_len(&forged_stage).is_err());
        let forged_seal =
            ZkAmsMkheDirectObjectSealTokenV1::from_staging(forged_stage, [0xB8; 32]).unwrap();
        assert!(reopened.sealed_len(&forged_seal).is_err());
        assert!(
            ZkAmsMkheDirectObjectStagingTokenV1::new(
                [0; 32],
                [1; 32],
                ZkAmsMkheDirectObjectKindV1::CpkPartyB,
                9,
            )
            .is_err()
        );
    }

    #[test]
    fn truncated_and_mutated_seals_fail_before_publication() {
        let root = TestRoot::new();
        let payload = fixture_payload(1_025, 0x71);
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let seal = stage_and_seal(&mut store, ZkAmsMkheDirectObjectKindV1::CpkPartyB, &payload);
        let sealed_path = store
            .seal_directory(seal.seal_identity())
            .join(PAYLOAD_FILE_V1);
        overwrite_test_file(&sealed_path, &payload[..payload.len() - 1]);
        assert!(store.sealed_len(&seal).is_err());
        let pointer = ZkAmsMkheDirectObjectPointerV1::from_payload(seal.kind(), &payload).unwrap();
        assert!(store.publish_sealed_by_pointer(&seal, pointer).is_err());

        let second = stage_and_seal(&mut store, ZkAmsMkheDirectObjectKindV1::CpkPartyB, &payload);
        let second_path = store
            .seal_directory(second.seal_identity())
            .join(PAYLOAD_FILE_V1);
        let mut mutated = payload.clone();
        mutated[payload.len() / 2] ^= 0x80;
        overwrite_test_file(&second_path, &mutated);
        assert!(store.publish_sealed_by_pointer(&second, pointer).is_err());
    }

    #[test]
    fn published_tampering_and_writable_permissions_fail_closed() {
        let root = TestRoot::new();
        let payload = fixture_payload(4_097, 0x81);
        for attack in 0..4 {
            let case_root = root.0.join(format!("case-{attack}"));
            create_private_directory(&case_root).unwrap();
            let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&case_root).unwrap();
            let pointer = publish(
                &mut store,
                ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
                &payload,
            )
            .pointer();
            let object = store.object_path(pointer);
            match attack {
                0 => overwrite_test_file(&object, &payload[..payload.len() - 1]),
                1 => {
                    let mut mutated = payload.clone();
                    mutated[17] ^= 1;
                    overwrite_test_file(&object, &mutated);
                }
                2 => {
                    let original = object.with_file_name("original");
                    fs::rename(&object, original).unwrap();
                    create_immutable_file(&object, &fixture_payload(payload.len(), 0x82)).unwrap();
                }
                3 => make_test_file_mutable(&object),
                _ => unreachable!(),
            }
            assert!(store.lookup_published_pointer(pointer).is_err());
            assert!(
                validate_zk_ams_mkhe_direct_object_v1(pointer.kind(), pointer, &mut store).is_err()
            );
        }
    }

    #[test]
    fn read_bounds_unknown_pointer_and_short_object_are_rejected() {
        let root = TestRoot::new();
        let payload = fixture_payload(32, 0x91);
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let pointer =
            publish(&mut store, ZkAmsMkheDirectObjectKindV1::CpkPartyB, &payload).pointer();
        assert!(store.read_at(pointer, 0, &mut []).is_err());
        assert!(store.read_at(pointer, 31, &mut [0; 2]).is_err());
        assert!(
            store
                .read_at(
                    pointer,
                    0,
                    &mut vec![0; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 1]
                )
                .is_err()
        );
        let unknown = ZkAmsMkheDirectObjectPointerV1::from_payload(
            pointer.kind(),
            &fixture_payload(32, 0x92),
        )
        .unwrap();
        assert!(store.lookup_published_pointer(unknown).unwrap().is_none());
        overwrite_test_file(&store.object_path(pointer), &payload[..31]);
        assert!(store.object_len(pointer).is_err());
        assert!(store.read_at(pointer, 0, &mut [0; 32]).is_err());
    }

    #[test]
    fn malformed_records_names_and_unknown_entries_fail_closed() {
        let record_root = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&record_root.0).unwrap();
        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 4)
            .unwrap();
        let record = store
            .staging_directory(stage.staging_identity())
            .join(RECORD_FILE_V1);
        make_test_file_mutable(&record);
        let mut file = fs::OpenOptions::new().write(true).open(&record).unwrap();
        file.seek(SeekFrom::Start(17)).unwrap();
        file.write_all(&[0xFF]).unwrap();
        file.sync_all().unwrap();
        make_file_immutable(&record).unwrap();
        assert!(store.staged_len(&stage).is_err());

        let unknown_root = TestRoot::new();
        let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&unknown_root.0).unwrap();
        create_immutable_file(&store.layout.join("unexpected-entry"), b"unknown").unwrap();
        drop(store);
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&unknown_root.0).is_err());

        let name_root = TestRoot::new();
        let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&name_root.0).unwrap();
        create_immutable_file(&store.objects.join("ABCDEF"), b"bad").unwrap();
        drop(store);
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&name_root.0).is_err());
    }

    #[test]
    fn restart_tolerates_incomplete_identity_named_orphan() {
        let root = TestRoot::new();
        let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        create_private_directory(&store.staging.join(hex::encode([0xC3; 32]))).unwrap();
        drop(store);
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).is_ok());
    }

    #[test]
    fn retained_layout_descriptor_prevents_ancestor_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let escaped = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let displaced_layout = root.0.join("displaced-layout");
        fs::rename(&store.layout, &displaced_layout).unwrap();
        create_private_directory(&escaped.0.join(STAGING_DIRECTORY_V1)).unwrap();
        symlink(&escaped.0, &store.layout).unwrap();

        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 4)
            .unwrap();
        assert_eq!(store.write_staged_at(&stage, 0, b"safe").unwrap(), 4);
        assert!(
            displaced_layout
                .join(STAGING_DIRECTORY_V1)
                .join(hex::encode(stage.staging_identity()))
                .join(PAYLOAD_FILE_V1)
                .is_file()
        );
        assert_eq!(
            fs::read_dir(escaped.0.join(STAGING_DIRECTORY_V1))
                .unwrap()
                .count(),
            0,
            "the substituted layout target must remain untouched"
        );

        fs::remove_file(&store.layout).unwrap();
        fs::rename(displaced_layout, &store.layout).unwrap();
    }

    #[test]
    fn retained_root_and_layout_handles_survive_root_name_replacement_without_escape() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let escaped = TestRoot::new();
        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let displaced_root = root.0.with_file_name(format!(
            "{}-displaced",
            root.0.file_name().unwrap().to_string_lossy()
        ));
        fs::rename(&root.0, &displaced_root).unwrap();
        create_private_directory(&escaped.0.join(LAYOUT_DIRECTORY_V1)).unwrap();
        create_private_directory(
            &escaped
                .0
                .join(LAYOUT_DIRECTORY_V1)
                .join(STAGING_DIRECTORY_V1),
        )
        .unwrap();
        symlink(&escaped.0, &root.0).unwrap();

        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 3)
            .unwrap();
        assert_eq!(store.write_staged_at(&stage, 0, b"pin").unwrap(), 3);
        assert!(
            displaced_root
                .join(LAYOUT_DIRECTORY_V1)
                .join(STAGING_DIRECTORY_V1)
                .join(hex::encode(stage.staging_identity()))
                .join(PAYLOAD_FILE_V1)
                .is_file()
        );
        assert_eq!(
            fs::read_dir(
                escaped
                    .0
                    .join(LAYOUT_DIRECTORY_V1)
                    .join(STAGING_DIRECTORY_V1)
            )
            .unwrap()
            .count(),
            0
        );

        fs::remove_file(&root.0).unwrap();
        fs::rename(displaced_root, &root.0).unwrap();
    }

    #[test]
    fn concurrent_publication_after_layout_replacement_is_atomic_and_cannot_escape() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let escaped = TestRoot::new();
        let stores: Vec<_> = (0..4)
            .map(|_| ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap())
            .collect();
        let layout = root.0.join(LAYOUT_DIRECTORY_V1);
        let displaced_layout = root.0.join("displaced-layout");
        fs::rename(&layout, &displaced_layout).unwrap();
        for name in [
            STAGING_DIRECTORY_V1,
            SEALED_DIRECTORY_V1,
            OBJECTS_DIRECTORY_V1,
        ] {
            create_private_directory(&escaped.0.join(name)).unwrap();
        }
        symlink(&escaped.0, &layout).unwrap();

        let payload = Arc::new(fixture_payload(16_417, 0xB4));
        let barrier = Arc::new(Barrier::new(4));
        let workers: Vec<_> = stores
            .into_iter()
            .map(|mut store| {
                let payload = Arc::clone(&payload);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    publish(
                        &mut store,
                        ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
                        &payload,
                    )
                    .pointer()
                })
            })
            .collect();
        let pointers: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(pointers.iter().all(|pointer| *pointer == pointers[0]));
        assert_eq!(
            fs::read_dir(displaced_layout.join(OBJECTS_DIRECTORY_V1))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            fs::read_dir(escaped.0.join(OBJECTS_DIRECTORY_V1))
                .unwrap()
                .count(),
            0
        );

        fs::remove_file(&layout).unwrap();
        fs::rename(displaced_layout, layout).unwrap();
    }

    #[test]
    fn initial_layout_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let root = TestRoot::new();
        let escaped = TestRoot::new();
        let store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).unwrap();
        let layout = store.layout.clone();
        drop(store);
        let displaced_layout = root.0.join("displaced-layout");
        fs::rename(&layout, &displaced_layout).unwrap();
        symlink(&escaped.0, &layout).unwrap();
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).is_err());
        assert_eq!(fs::read_dir(&escaped.0).unwrap().count(), 0);
        fs::remove_file(&layout).unwrap();
        fs::rename(displaced_layout, layout).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn root_stage_and_published_symlinks_are_never_followed() {
        use std::os::unix::fs::symlink;

        let real_root = TestRoot::new();
        let link_parent = TestRoot::new();
        let root_link = link_parent.0.join("store-link");
        symlink(&real_root.0, &root_link).unwrap();
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&root_link).is_err());

        let mut store = ZkAmsMkheDirectObjectFilesystemCasV1::open(&real_root.0).unwrap();
        let stage = store
            .begin_staging(ZkAmsMkheDirectObjectKindV1::CpkPartyB, 4)
            .unwrap();
        let stage_payload = store
            .staging_directory(stage.staging_identity())
            .join(PAYLOAD_FILE_V1);
        fs::remove_file(&stage_payload).unwrap();
        symlink("/dev/null", &stage_payload).unwrap();
        assert!(store.staged_len(&stage).is_err());

        let pointer = publish(
            &mut store,
            ZkAmsMkheDirectObjectKindV1::CpkRelationProof,
            &fixture_payload(64, 0xA1),
        )
        .pointer();
        let object = store.object_path(pointer);
        let preserved = object.with_file_name("preserved-object");
        fs::rename(&object, &preserved).unwrap();
        symlink(&preserved, &object).unwrap();
        assert!(store.lookup_published_pointer(pointer).is_err());
        assert!(store.read_at(pointer, 0, &mut [0; 1]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn writable_by_others_root_is_rejected() {
        use std::os::unix::fs::PermissionsExt;

        let root = TestRoot::new();
        let mut permissions = fs::metadata(&root.0).unwrap().permissions();
        permissions.set_mode(0o777);
        fs::set_permissions(&root.0, permissions).unwrap();
        assert!(ZkAmsMkheDirectObjectFilesystemCasV1::open(&root.0).is_err());
    }
}
